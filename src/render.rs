//! Reusable GDI+ paths and brushes render directly into the layered-window DIB.
use std::{ffi::c_void, ptr::null_mut};
type Handle = *mut c_void;
#[repr(C)]
struct Startup { version:u32, callback:Handle, background:i32, codecs:i32 }
#[repr(C)]
struct Point { x:f32, y:f32 }
// Load the system DLL once; GNU Rust installations need no extra import library.
static MODULE:std::sync::LazyLock<usize>=std::sync::LazyLock::new(|| unsafe {let name:Vec<u16>="gdiplus.dll\0".encode_utf16().collect();let module=crate::LoadLibraryW(name.as_ptr());assert!(!module.is_null(),"GDI+ unavailable");module as usize});
macro_rules! gdiplus {
    ($(fn $name:ident($($arg:ident:$ty:ty),*) $(->$ret:ty)?;)*) => {$(
        #[allow(non_snake_case)]
        unsafe fn $name($($arg:$ty),*) $(->$ret)? {
            static FUNCTION:std::sync::LazyLock<unsafe extern "system" fn($($ty),*) $(->$ret)?>=std::sync::LazyLock::new(|| unsafe {
                let address=crate::GetProcAddress(*MODULE as Handle,concat!(stringify!($name),"\0").as_ptr());
                assert!(!address.is_null(),"Missing GDI+ export");std::mem::transmute(address)
            });
            FUNCTION($($arg),*)
        }
    )*};
}
gdiplus! {
    fn GdiplusStartup(token:*mut usize,input:*const Startup,output:Handle)->i32;
    fn GdiplusShutdown(token:usize);
    fn GdipCreateBitmapFromScan0(w:i32,h:i32,stride:i32,format:i32,pixels:*mut u8,image:*mut Handle)->i32;
    fn GdipGetImageGraphicsContext(image:Handle,graphics:*mut Handle)->i32;
    fn GdipDeleteGraphics(graphics:Handle)->i32;
    fn GdipDisposeImage(image:Handle)->i32;
    fn GdipSetSmoothingMode(graphics:Handle,mode:i32)->i32;
    fn GdipSetPixelOffsetMode(graphics:Handle,mode:i32)->i32;
    fn GdipCreateSolidFill(color:u32,brush:*mut Handle)->i32;
    fn GdipSetSolidFillColor(brush:Handle,color:u32)->i32;
    fn GdipDeleteBrush(brush:Handle)->i32;
    fn GdipCreatePath(mode:i32,path:*mut Handle)->i32;
    fn GdipResetPath(path:Handle)->i32;
    fn GdipDeletePath(path:Handle)->i32;
    fn GdipAddPathArc(path:Handle,x:f32,y:f32,w:f32,h:f32,start:f32,sweep:f32)->i32;
    fn GdipAddPathRectangle(path:Handle,x:f32,y:f32,w:f32,h:f32)->i32;
    fn GdipAddPathPolygon(path:Handle,points:*const Point,count:i32)->i32;
    fn GdipClosePathFigure(path:Handle)->i32;
    fn GdipFillPath(graphics:Handle,brush:Handle,path:Handle)->i32;
    fn GdipCreateTexture(image:Handle,wrap:i32,brush:*mut Handle)->i32;
    fn GdipResetTextureTransform(brush:Handle)->i32;
    fn GdipScaleTextureTransform(brush:Handle,x:f32,y:f32,order:i32)->i32;
    fn GdipTranslateTextureTransform(brush:Handle,x:f32,y:f32,order:i32)->i32;
    fn GdipSetInterpolationMode(graphics:Handle,mode:i32)->i32;
    fn GdipFlush(graphics:Handle,intention:i32)->i32;
    fn GdipCreateImageAttributes(imageattr:*mut Handle)->i32;
    fn GdipSetImageAttributesColorMatrix(imageattr:Handle,colortype:i32,enable:i32,colormatrix:*const f32,graymatrix:*const f32,flags:i32)->i32;
    fn GdipDisposeImageAttributes(imageattr:Handle)->i32;
    fn GdipSetClipPath(graphics:Handle,path:Handle,combineMode:i32)->i32;
    fn GdipResetClip(graphics:Handle)->i32;
    fn GdipCreatePen1(color:u32,width:f32,unit:i32,pen:*mut Handle)->i32;
    fn GdipDeletePen(pen:Handle)->i32;
    fn GdipSetPenColor(pen:Handle,color:u32)->i32;
    fn GdipSetPenWidth(pen:Handle,width:f32)->i32;
    fn GdipSetPenStartCap(pen:Handle,cap:i32)->i32;
    fn GdipSetPenEndCap(pen:Handle,cap:i32)->i32;
    fn GdipSetPenLineJoin(pen:Handle,join:i32)->i32;
    fn GdipDrawPath(graphics:Handle,pen:Handle,path:Handle)->i32;
    fn GdipAddPathLine2(path:Handle,points:*const Point,count:i32)->i32;
    fn GdipAddPathBezier(path:Handle,x1:f32,y1:f32,x2:f32,y2:f32,x3:f32,y3:f32,x4:f32,y4:f32)->i32;
    fn GdipAddPathEllipse(path:Handle,x:f32,y:f32,w:f32,h:f32)->i32;
    fn GdipDrawImageRectRect(graphics:Handle,image:Handle,dstx:f32,dsty:f32,dstwidth:f32,dstheight:f32,srcx:f32,srcy:f32,srcwidth:f32,srcheight:f32,srcUnit:i32,imageAttributes:Handle,callback:Handle,callbackData:Handle)->i32;
}
/// The rounded artwork already scaled to its on-screen pixel size (premultiplied BGRA), reused while
/// the size and sub-pixel phase stay the same: bicubic texture scaling cost up to ~0.4 ms a frame.
struct ArtCache { key:(i32,i32,i32,i32), n:usize, pixels:Vec<u32> }
pub struct Renderer { target:*mut u32, target_w:usize, target_h:usize, art_cache:std::cell::RefCell<Option<ArtCache>>, token:usize, image:Handle, graphics:Handle, brush:Handle, pen:Handle, path:Handle, scale:f32, art:Handle, texture:Handle, attr:Handle, art_pixels:Vec<u32>, tint:Option<u32>, icon:Handle, icon_pixels:Vec<u32> }
impl Renderer {
    pub unsafe fn new(pixels:*mut u32,width:usize,height:i32,scale:f64)->Self {
        let mut r=Self {target:pixels,target_w:width,target_h:height.unsigned_abs() as usize,art_cache:Default::default(),token:0,image:null_mut(),graphics:null_mut(),brush:null_mut(),pen:null_mut(),path:null_mut(),scale:scale as f32,art:null_mut(),texture:null_mut(),attr:null_mut(),art_pixels:Vec::new(),tint:None,icon:null_mut(),icon_pixels:Vec::new()};
        assert_eq!(GdiplusStartup(&mut r.token,&Startup{version:1,callback:null_mut(),background:0,codecs:0},null_mut()),0);
        assert_eq!(GdipCreateBitmapFromScan0(width as i32,height,width as i32*4,0x22009,pixels.cast(),&mut r.image),0);
        assert_eq!(GdipGetImageGraphicsContext(r.image,&mut r.graphics),0);
        // 8x8 antialiasing where GDI+ 1.1 offers it, else the standard 8x4 mode.
        if GdipSetSmoothingMode(r.graphics,5)!=0 {GdipSetSmoothingMode(r.graphics,4);}
        GdipSetPixelOffsetMode(r.graphics,4);GdipSetInterpolationMode(r.graphics,7);
        // Round caps and joins, like SF Symbols strokes.
        GdipCreatePen1(0xff000000,1.,0,&mut r.pen);GdipSetPenStartCap(r.pen,2);GdipSetPenEndCap(r.pen,2);GdipSetPenLineJoin(r.pen,2);
        GdipCreateSolidFill(0xff000000,&mut r.brush);GdipCreatePath(0,&mut r.path);assert_eq!(GdipCreateImageAttributes(&mut r.attr),0);r
    }
    unsafe fn rounded(&self,x:f64,y:f64,w:f64,h:f64,radius:f64) {
        GdipResetPath(self.path);
        let [x,y,w,h,r]=[x,y,w,h,radius.min(w/2.).min(h/2.)].map(|n|n as f32*self.scale);
        if r<=0. {GdipAddPathRectangle(self.path,x,y,w,h);return;}
        let d=2.*r;
        for (cx,cy,start) in [(x,y,180.),(x+w-d,y,270.),(x+w-d,y+h-d,0.),(x,y+h-d,90.)] {GdipAddPathArc(self.path,cx,cy,d,d,start,90.);}
        GdipClosePathFigure(self.path);
    }
    unsafe fn fill(&self,color:u32) {
        GdipSetSolidFillColor(self.brush,argb(color));
        GdipFillPath(self.graphics,self.brush,self.path);GdipFlush(self.graphics,1);
    }
    pub unsafe fn round(&self,x:f64,y:f64,w:f64,h:f64,r:f64,color:u32) {
        if w<=0. || h<=0. {return;} self.rounded(x,y,w,h,r);self.fill(color);
    }
    /// Filled annulus segment: `sweep` degrees clockwise from `start` (0 = +x, -90 = top).
    pub unsafe fn ring(&self,cx:f64,cy:f64,r:f64,w:f64,start:f64,sweep:f64,color:u32) {
        if sweep.abs()<0.05 || w<=0. {return;}
        let sc=self.scale;let (cx,cy,ro,ri)=(cx as f32*sc,cy as f32*sc,r as f32*sc,(r-w).max(0.) as f32*sc);
        GdipResetPath(self.path);
        GdipAddPathArc(self.path,cx-ro,cy-ro,2.*ro,2.*ro,start as f32,sweep as f32);
        GdipAddPathArc(self.path,cx-ri,cy-ri,2.*ri,2.*ri,(start+sweep) as f32,-sweep as f32);
        GdipClosePathFigure(self.path);
        self.fill(color);
    }
    unsafe fn draw(&self,width:f64,color:u32) {
        GdipSetPenColor(self.pen,argb(color));GdipSetPenWidth(self.pen,width as f32*self.scale);
        GdipDrawPath(self.graphics,self.pen,self.path);GdipFlush(self.graphics,1);
    }
    /// Open (or closed) polyline stroke.
    pub unsafe fn stroke(&self,points:&[(f64,f64)],closed:bool,width:f64,color:u32) {
        let points:Vec<Point>=points.iter().map(|&(x,y)|Point{x:x as f32*self.scale,y:y as f32*self.scale}).collect();
        GdipResetPath(self.path);GdipAddPathLine2(self.path,points.as_ptr(),points.len() as i32);
        if closed {GdipClosePathFigure(self.path);}
        self.draw(width,color);
    }
    pub unsafe fn stroke_bezier(&self,p:[(f64,f64);4],width:f64,color:u32) {
        let s=self.scale as f64;let f=|v:f64|(v*s) as f32;
        GdipResetPath(self.path);GdipAddPathBezier(self.path,f(p[0].0),f(p[0].1),f(p[1].0),f(p[1].1),f(p[2].0),f(p[2].1),f(p[3].0),f(p[3].1));
        self.draw(width,color);
    }
    pub unsafe fn stroke_round(&self,x:f64,y:f64,w:f64,h:f64,radius:f64,width:f64,color:u32) {
        self.rounded(x,y,w,h,radius);self.draw(width,color);
    }
    pub unsafe fn stroke_ellipse(&self,cx:f64,cy:f64,rx:f64,ry:f64,width:f64,color:u32) {
        let s=self.scale;
        GdipResetPath(self.path);GdipAddPathEllipse(self.path,(cx-rx) as f32*s,(cy-ry) as f32*s,2.*rx as f32*s,2.*ry as f32*s);
        self.draw(width,color);
    }
    pub unsafe fn ellipse(&self,cx:f64,cy:f64,rx:f64,ry:f64,color:u32) {
        let s=self.scale;
        GdipResetPath(self.path);GdipAddPathEllipse(self.path,(cx-rx) as f32*s,(cy-ry) as f32*s,2.*rx as f32*s,2.*ry as f32*s);
        self.fill(color);
    }
    pub unsafe fn polygon<const N:usize>(&self,points:[(f64,f64);N],color:u32) {
        let points=points.map(|(x,y)|Point{x:x as f32*self.scale,y:y as f32*self.scale});
        GdipResetPath(self.path);GdipAddPathPolygon(self.path,points.as_ptr(),N as i32);self.fill(color);
    }
    pub unsafe fn set_artwork(&mut self,pixels:Option<&[u32]>) {
        self.clear_art();self.art_cache.replace(None);
        if let Some(pixels)=pixels {
            self.art_pixels.clear();self.art_pixels.extend_from_slice(pixels);
            GdipCreateBitmapFromScan0(136,136,544,0x22009,self.art_pixels.as_mut_ptr().cast(),&mut self.art);
            GdipCreateTexture(self.art,4,&mut self.texture);
            self.tint=accent(pixels);
        }
    }
    /// Saturation-weighted artwork color, lifted so it reads on black (COLORREF order).
    pub fn tint(&self)->Option<u32> {self.tint}
    pub unsafe fn artwork(&self,x:f64,y:f64,size:f64,radius:f64,opacity:f64)->bool {
        if self.art.is_null() || opacity<=0. {return false;}
        if opacity>=1.0 && !self.texture.is_null() {
            let (px,py,ps)=(x*self.scale as f64,y*self.scale as f64,size*self.scale as f64);
            let (ix,iy)=(px.floor(),py.floor());
            let (qx,qy)=(((px-ix)*4.).round().min(3.),((py-iy)*4.).round().min(3.));
            let key=((ps*4.).round() as i32,(radius*self.scale as f64*4.).round() as i32,qx as i32,qy as i32);
            let mut cached=self.art_cache.borrow_mut();
            if cached.as_ref().map_or(true,|c|c.key!=key) {*cached=self.render_art(key,ps,radius,qx/4.,qy/4.);}
            if let Some(cache)=cached.as_ref() {
                self.blit_premultiplied(&cache.pixels,cache.n,ix as i64-1,iy as i64-1);
                return true;
            }
        }
        self.rounded(x,y,size,size,radius);
        if opacity>=1.0 {
            if !self.texture.is_null() {
                GdipResetTextureTransform(self.texture);
                GdipScaleTextureTransform(self.texture,(size as f32*self.scale)/136.,(size as f32*self.scale)/136.,1);
                GdipTranslateTextureTransform(self.texture,x as f32*self.scale,y as f32*self.scale,1);
                GdipFillPath(self.graphics,self.texture,self.path);
            }
        } else if !self.attr.is_null() {
            let matrix:[f32;25]=[
                1.,0.,0.,0.,0.,
                0.,1.,0.,0.,0.,
                0.,0.,1.,0.,0.,
                0.,0.,0.,opacity as f32,0.,
                0.,0.,0.,0.,1.,
            ];
            GdipSetImageAttributesColorMatrix(self.attr,0,1,matrix.as_ptr(),null_mut(),0);
            GdipSetClipPath(self.graphics,self.path,0);
            let (dx,dy,ds)=(x as f32*self.scale,y as f32*self.scale,size as f32*self.scale);
            GdipDrawImageRectRect(self.graphics,self.art,dx,dy,ds,ds,0.,0.,136.,136.,2,self.attr,null_mut(),null_mut());
            GdipResetClip(self.graphics);
        }
        GdipFlush(self.graphics,1);true
    }
    /// Notification icon: `ICON_PX`² premultiplied BGRA with real transparency.
    pub unsafe fn set_icon(&mut self,pixels:Option<&[u32]>) {
        if !self.icon.is_null() {GdipDisposeImage(self.icon);self.icon=null_mut();}
        let n=crate::notify::ICON_PX;
        if let Some(pixels)=pixels.filter(|p|p.len()==n*n) {
            self.icon_pixels.clear();self.icon_pixels.extend_from_slice(pixels);
            GdipCreateBitmapFromScan0(n as i32,n as i32,n as i32*4,0xE200B,self.icon_pixels.as_mut_ptr().cast(),&mut self.icon);
        }
    }
    /// Draws the notification icon, clipped to a rounded square (`radius` 0 keeps the icon's own shape).
    pub unsafe fn icon(&self,x:f64,y:f64,size:f64,radius:f64,opacity:f64)->bool {
        if self.icon.is_null() || self.attr.is_null() || opacity<=0. {return false;}
        let matrix:[f32;25]=[1.,0.,0.,0.,0., 0.,1.,0.,0.,0., 0.,0.,1.,0.,0., 0.,0.,0.,opacity.min(1.) as f32,0., 0.,0.,0.,0.,1.];
        GdipSetImageAttributesColorMatrix(self.attr,0,1,matrix.as_ptr(),null_mut(),0);
        if radius>0. {self.rounded(x,y,size,size,radius);GdipSetClipPath(self.graphics,self.path,0);}
        let n=crate::notify::ICON_PX as f32;
        let (dx,dy,ds)=(x as f32*self.scale,y as f32*self.scale,size as f32*self.scale);
        GdipDrawImageRectRect(self.graphics,self.icon,dx,dy,ds,ds,0.,0.,n,n,2,self.attr,null_mut(),null_mut());
        if radius>0. {GdipResetClip(self.graphics);}
        GdipFlush(self.graphics,1);true
    }
    /// Fills the rounded artwork into its own small premultiplied bitmap at the given sub-pixel phase.
    unsafe fn render_art(&self,key:(i32,i32,i32,i32),ps:f64,radius:f64,fx:f64,fy:f64)->Option<ArtCache> {
        if ps<=0. || ps>1024. {return None;}
        let n=ps.ceil() as usize+3;
        let mut pixels=vec![0u32;n*n];
        let mut bitmap=null_mut();
        if GdipCreateBitmapFromScan0(n as i32,n as i32,n as i32*4,0xE200B,pixels.as_mut_ptr().cast(),&mut bitmap)!=0 || bitmap.is_null() {return None;}
        let mut graphics=null_mut();
        if GdipGetImageGraphicsContext(bitmap,&mut graphics)!=0 || graphics.is_null() {GdipDisposeImage(bitmap);return None;}
        if GdipSetSmoothingMode(graphics,5)!=0 {GdipSetSmoothingMode(graphics,4);}
        GdipSetPixelOffsetMode(graphics,4);GdipSetInterpolationMode(graphics,7);
        let sc=self.scale as f64;
        // Drawn one pixel in from the corner, plus the phase, so it lands exactly where a direct fill would.
        let (ox,oy)=(1.+fx,1.+fy);
        self.rounded(ox/sc,oy/sc,ps/sc,ps/sc,radius);
        GdipResetTextureTransform(self.texture);
        GdipScaleTextureTransform(self.texture,ps as f32/136.,ps as f32/136.,1);
        GdipTranslateTextureTransform(self.texture,ox as f32,oy as f32,1);
        GdipFillPath(graphics,self.texture,self.path);
        GdipFlush(graphics,1);GdipDeleteGraphics(graphics);GdipDisposeImage(bitmap);
        Some(ArtCache{key,n,pixels})
    }
    /// Source-over blend of premultiplied pixels into the frame at (`left`, `top`), clipped to it.
    unsafe fn blit_premultiplied(&self,src:&[u32],n:usize,left:i64,top:i64) {
        let frame=std::slice::from_raw_parts_mut(self.target,self.target_w*self.target_h);
        for row in 0..n as i64 {
            let y=top+row;
            if y<0 || y>=self.target_h as i64 {continue;}
            for col in 0..n as i64 {
                let x=left+col;
                if x<0 || x>=self.target_w as i64 {continue;}
                let p=src[(row as usize)*n+col as usize];
                let a=p>>24;
                if a==0 {continue;}
                let d=&mut frame[y as usize*self.target_w+x as usize];
                if a==255 {*d=p;continue;}
                let k=255-a;
                let ch=|shift:u32|(((p>>shift)&255)+(((*d>>shift)&255)*k+127)/255).min(255)<<shift;
                *d=0xff000000|ch(16)|ch(8)|ch(0);
            }
        }
    }
    unsafe fn clear_art(&mut self) {self.tint=None;if !self.texture.is_null(){GdipDeleteBrush(self.texture);self.texture=null_mut();}if !self.art.is_null(){GdipDisposeImage(self.art);self.art=null_mut();}}
}
/// COLORREF (0x00BBGGRR) to opaque GDI+ ARGB.
fn argb(color:u32)->u32 {0xff000000|((color&255)<<16)|(color&0xff00)|((color>>16)&255)}
fn accent(pixels:&[u32])->Option<u32> {
    let (mut r,mut g,mut b,mut total)=(0.,0.,0.,0.);
    for &p in pixels {
        let c=[((p>>16)&255) as f64,((p>>8)&255) as f64,(p&255) as f64];
        let sat=c.iter().cloned().fold(0.,f64::max)-c.iter().cloned().fold(255.,f64::min);
        let w=sat*sat+1.;r+=c[0]*w;g+=c[1]*w;b+=c[2]*w;total+=w;
    }
    if total==0. {return None;}
    let (r,g,b)=(r/total,g/total,b/total);
    let peak=r.max(g).max(b).max(1.);
    let lift=|c:f64|((c*225./peak)*0.82+255.*0.18).round().min(255.) as u32;
    Some(lift(r)|lift(g)<<8|lift(b)<<16)
}
impl Drop for Renderer { fn drop(&mut self) {unsafe {if !self.attr.is_null(){GdipDisposeImageAttributes(self.attr);}self.clear_art();self.set_icon(None);GdipDeletePath(self.path);if !self.pen.is_null(){GdipDeletePen(self.pen);}GdipDeleteBrush(self.brush);GdipDeleteGraphics(self.graphics);GdipDisposeImage(self.image);GdiplusShutdown(self.token);}} }

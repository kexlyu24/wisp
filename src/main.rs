#![windows_subsystem = "windows"]

mod media;
mod render;
mod tasks;
mod lyrics;
mod volume;
mod tray;
mod notify;
mod settings;
use std::{ffi::c_void, mem::size_of, ptr::null_mut};
type Handle = *mut c_void;
type Hwnd = Handle;
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Point { x: i32, y: i32 }
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Rect { left: i32, top: i32, right: i32, bottom: i32 }
#[repr(C)]
struct Message { hwnd: Hwnd, message: u32, wparam: usize, lparam: isize, time: u32, pt: Point, private: u32 }
#[repr(C)]
struct WindowClass { size: u32, style: u32, proc: unsafe extern "system" fn(Hwnd,u32,usize,isize)->isize, cls_extra: i32, wnd_extra: i32, instance: Handle, icon: Handle, cursor: Handle, background: Handle, menu: *const u16, name: *const u16, small_icon: Handle }
#[repr(C)]
struct Paint { dc: Handle, erase: i32, rect: Rect, restore: i32, update: i32, reserved: [u8;32] }
#[repr(C)]
struct BitmapInfo { size:u32, width:i32, height:i32, planes:u16, bits:u16, compression:u32, bytes:u32, x:i32, y:i32, used:u32, important:u32 }
#[repr(C)]
struct Size {x:i32,y:i32}
#[repr(C)]
struct Blend {op:u8,flags:u8,alpha:u8,format:u8}
#[repr(C)]
#[derive(Default)]
struct Time {year:u16,month:u16,weekday:u16,day:u16,hour:u16,minute:u16,second:u16,millis:u16}
#[repr(C)]
struct MouseTracking {size:u32,flags:u32,hwnd:Hwnd,hover_time:u32}
#[link(name="user32")]
extern "system" {
    fn TrackMouseEvent(event:*mut MouseTracking)->i32;
    fn MessageBeep(kind:u32)->i32;
    fn SetFocus(hwnd:Hwnd)->Hwnd;
    fn SystemParametersInfoW(action:u32,param:u32,value:*mut c_void,flags:u32)->i32;
    fn UpdateLayeredWindow(hwnd:Hwnd,dst:Handle,pos:*const Point,size:*const Size,src:Handle,origin:*const Point,key:u32,blend:*const Blend,flags:u32)->i32;
    fn SendMessageW(hwnd:Hwnd,msg:u32,w:usize,l:isize)->isize;
    fn UpdateWindow(hwnd:Hwnd)->i32;
    fn RegisterClassExW(class:*const WindowClass)->u16;
    fn CreateWindowExW(ex:u32,class:*const u16,title:*const u16,style:u32,x:i32,y:i32,w:i32,h:i32,parent:Handle,menu:Handle,instance:Handle,param:*mut c_void)->Hwnd;
    fn DefWindowProcW(hwnd:Hwnd,msg:u32,w:usize,l:isize)->isize;
    fn GetMessageW(msg:*mut Message,hwnd:Hwnd,min:u32,max:u32)->i32;
    fn TranslateMessage(msg:*const Message)->i32;
    fn DispatchMessageW(msg:*const Message)->isize;
    fn PostQuitMessage(code:i32);
    fn DestroyWindow(hwnd:Hwnd)->i32;
    fn ShowWindow(hwnd:Hwnd,cmd:i32)->i32;
    fn SetWindowPos(hwnd:Hwnd,after:Hwnd,x:i32,y:i32,w:i32,h:i32,flags:u32)->i32;
    fn GetClientRect(hwnd:Hwnd,rect:*mut Rect)->i32;
    fn GetWindowRect(hwnd:Hwnd,rect:*mut Rect)->i32;
    fn InvalidateRect(hwnd:Hwnd,rect:*const Rect,erase:i32)->i32;
    fn BeginPaint(hwnd:Hwnd,paint:*mut Paint)->Handle;
    fn EndPaint(hwnd:Hwnd,paint:*const Paint)->i32;
    fn DrawTextW(dc:Handle,text:*const u16,len:i32,rect:*mut Rect,flags:u32)->i32;
    fn LoadCursorW(instance:Handle,name:*const u16)->Handle;
    fn GetSystemMetrics(index:i32)->i32;
    fn SetProcessDPIAware()->i32;
    fn GetDpiForWindow(hwnd:Hwnd)->u32;
    fn SetWindowLongPtrW(hwnd:Hwnd,index:i32,value:isize)->isize;
    fn GetWindowLongPtrW(hwnd:Hwnd,index:i32)->isize;
    fn CreatePopupMenu()->Handle;
    fn AppendMenuW(menu:Handle,flags:u32,id:usize,text:*const u16)->i32;
    fn TrackPopupMenu(menu:Handle,flags:u32,x:i32,y:i32,reserved:i32,hwnd:Hwnd,rect:*const Rect)->i32;
    fn DestroyMenu(menu:Handle)->i32;
    fn GetCursorPos(point:*mut Point)->i32;
    fn SetForegroundWindow(hwnd:Hwnd)->i32;
    fn ChangeWindowMessageFilter(msg:u32,action:u32)->i32;
    fn SetCapture(hwnd:Hwnd)->Hwnd;
    fn ReleaseCapture()->i32;
}
#[link(name="gdi32")]
extern "system" {
    fn CreateDIBSection(dc:Handle,info:*const BitmapInfo,usage:u32,bits:*mut *mut c_void,section:Handle,offset:u32)->Handle;
    fn GdiFlush()->i32;
    fn IntersectClipRect(dc:Handle,left:i32,top:i32,right:i32,bottom:i32)->i32;
    fn SelectClipRgn(dc:Handle,region:Handle)->i32;
    fn GetTextExtentPoint32W(dc:Handle,text:*const u16,len:i32,size:*mut Size)->i32;
    fn GetTextFaceW(dc:Handle,count:i32,face:*mut u16)->i32;
    fn AddFontMemResourceEx(data:*const c_void,len:u32,reserved:*mut c_void,count:*mut u32)->Handle;
    fn DeleteObject(object:Handle)->i32;
    fn SelectObject(dc:Handle,object:Handle)->Handle;
    fn CreateCompatibleDC(dc:Handle)->Handle;
    fn DeleteDC(dc:Handle)->i32;
    fn SetBkMode(dc:Handle,mode:i32)->i32;
    fn SetTextColor(dc:Handle,color:u32)->u32;
    fn GetTextColor(dc:Handle)->u32;
    fn GetCurrentObject(dc:Handle,kind:u32)->Handle;
    fn GetClipBox(dc:Handle,rect:*mut Rect)->i32;
    fn CreateFontW(height:i32,width:i32,angle:i32,orientation:i32,weight:i32,italic:u32,underline:u32,strike:u32,charset:u32,out:u32,clip:u32,quality:u32,pitch:u32,face:*const u16)->Handle;
}
#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->Handle;
    fn LoadLibraryW(name:*const u16)->Handle;
    fn GetProcAddress(module:Handle,name:*const u8)->*mut c_void;
    fn GetLocalTime(time:*mut Time);
}
#[link(name="shell32")]
extern "system" {
    fn DragAcceptFiles(hwnd:Hwnd,accept:i32);
    fn DragQueryFileW(hdrop:Handle,ifile:u32,file:*mut u16,cch:u32)->u32;
    fn DragFinish(hdrop:Handle);
    fn ShellExecuteW(hwnd:Hwnd,op:*const u16,file:*const u16,params:*const u16,dir:*const u16,show:i32)->Handle;
    fn SHParseDisplayName(name:*const u16,bind:*mut c_void,pidl:*mut *mut c_void,attributes:u32,out:*mut u32)->i32;
    fn SHCreateDataObject(folder:*const c_void,count:u32,pidls:*const *mut c_void,inner:*mut c_void,riid:*const Guid,out:*mut *mut c_void)->i32;
    fn SHDoDragDrop(hwnd:Hwnd,data:*mut c_void,source:*mut c_void,effects:u32,effect:*mut u32)->i32;
    fn ILFree(pidl:*mut c_void);
}
#[repr(C)]
struct OpenFileNameW {
    struct_size:u32,hwnd_owner:Hwnd,instance:Handle,filter:*const u16,custom_filter:*mut u16,max_custom_filter:u32,filter_index:u32,file:*mut u16,max_file:u32,file_title:*mut u16,max_file_title:u32,initial_dir:*const u16,title:*const u16,flags:u32,file_offset:u16,file_extension:u16,def_ext:*const u16,cust_data:usize,hook:*mut c_void,template_name:*const u16,pv_reserved:*mut c_void,dw_reserved:u32,flags_ex:u32
}
#[link(name="comdlg32")]
extern "system" {
    fn GetOpenFileNameW(ofn:*mut OpenFileNameW)->i32;
}
#[link(name="ole32")]
extern "system" {
    fn OleInitialize(reserved:*mut c_void)->i32;
    fn OleUninitialize();
    fn RegisterDragDrop(hwnd:Hwnd,drop_target:*mut c_void)->i32;
    fn RevokeDragDrop(hwnd:Hwnd)->i32;
    fn ReleaseStgMedium(medium:*mut StgMedium);
}
#[repr(C)]
#[derive(Clone,Copy,Default)]
struct PointL { x:i32, y:i32 }
#[repr(C)]
#[derive(Clone,Copy,PartialEq,Eq)]
struct Guid { data1:u32, data2:u16, data3:u16, data4:[u8;8] }
const IID_IUNKNOWN: Guid = Guid { data1:0, data2:0, data3:0, data4:[0xc0,0,0,0,0,0,0,0x46] };
const IID_IDATAOBJECT: Guid = Guid { data1:0x0000010e, data2:0, data3:0, data4:[0xc0,0,0,0,0,0,0,0x46] };
const IID_IDROPTARGET: Guid = Guid { data1:0x00000122, data2:0, data3:0, data4:[0xc0,0,0,0,0,0,0,0x46] };
#[repr(C)]
struct FormatEtc { cf_format:u16, _pad:[u8;6], ptd:*mut c_void, dw_aspect:u32, lindex:i32, tymed:u32 }
#[repr(C)]
struct StgMedium { tymed:u32, _pad:u32, h_global:Handle, unk_for_release:*mut c_void }
#[repr(C)]
struct IDropTargetVtbl {
    query_interface: unsafe extern "system" fn(this:*mut DropTarget,riid:*const Guid,ppv:*mut *mut c_void)->i32,
    add_ref: unsafe extern "system" fn(this:*mut DropTarget)->u32,
    release: unsafe extern "system" fn(this:*mut DropTarget)->u32,
    drag_enter: unsafe extern "system" fn(this:*mut DropTarget,p_data_obj:*mut IDataObject,grf_key_state:u32,pt:PointL,pdw_effect:*mut u32)->i32,
    drag_over: unsafe extern "system" fn(this:*mut DropTarget,grf_key_state:u32,pt:PointL,pdw_effect:*mut u32)->i32,
    drag_leave: unsafe extern "system" fn(this:*mut DropTarget)->i32,
    drop: unsafe extern "system" fn(this:*mut DropTarget,p_data_obj:*mut IDataObject,grf_key_state:u32,pt:PointL,pdw_effect:*mut u32)->i32,
}
#[repr(C)]
struct DropTarget {
    lp_vtbl: *const IDropTargetVtbl,
    hwnd: Hwnd,
}
#[repr(C)]
struct IDataObjectVtbl {
    query_interface: unsafe extern "system" fn(this:*mut c_void,riid:*const Guid,ppv:*mut *mut c_void)->i32,
    add_ref: unsafe extern "system" fn(this:*mut c_void)->u32,
    release: unsafe extern "system" fn(this:*mut c_void)->u32,
    get_data: unsafe extern "system" fn(this:*mut c_void,pformatetc:*const FormatEtc,pmedium:*mut StgMedium)->i32,
}
#[repr(C)]
struct IDataObject {
    lp_vtbl: *const IDataObjectVtbl,
}
unsafe extern "system" fn dt_query_interface(this:*mut DropTarget,riid:*const Guid,ppv:*mut *mut c_void)->i32 {
    if ppv.is_null() { return 0x80070057u32 as i32; }
    if !riid.is_null() && (*riid == IID_IUNKNOWN || *riid == IID_IDROPTARGET) {
        *ppv = this as *mut c_void;
        1;
        return 0;
    }
    *ppv = null_mut();
    0x80004002u32 as i32
}
unsafe extern "system" fn dt_add_ref(_this:*mut DropTarget)->u32 { 1 }
unsafe extern "system" fn dt_release(_this:*mut DropTarget)->u32 { 1 }
unsafe extern "system" fn dt_drag_enter(this:*mut DropTarget,_p_data_obj:*mut IDataObject,_grf_key_state:u32,_pt:PointL,pdw_effect:*mut u32)->i32 {
    if !pdw_effect.is_null() { *pdw_effect = 1; }
    if !this.is_null() { SendMessageW((*this).hwnd,0x8004,0,0); }
    0
}
unsafe extern "system" fn dt_drag_over(this:*mut DropTarget,_grf_key_state:u32,pt:PointL,pdw_effect:*mut u32)->i32 {
    if !pdw_effect.is_null() { *pdw_effect = 1; }
    if !this.is_null() { SendMessageW((*this).hwnd,0x8007,pt.x as usize,pt.y as isize); }
    0
}
unsafe extern "system" fn dt_drag_leave(this:*mut DropTarget)->i32 {
    if !this.is_null() { SendMessageW((*this).hwnd,0x8005,0,0); }
    0
}
unsafe extern "system" fn dt_drop(this:*mut DropTarget,p_data_obj:*mut IDataObject,_grf_key_state:u32,_pt:PointL,pdw_effect:*mut u32)->i32 {
    if !this.is_null() {
        let paths = extract_hdrop_paths(p_data_obj);
        SendMessageW((*this).hwnd,0x8006,&paths as *const _ as usize,0);
    }
    if !pdw_effect.is_null() { *pdw_effect = 1; }
    0
}
static DROP_TARGET_VTBL: IDropTargetVtbl = IDropTargetVtbl {
    query_interface: dt_query_interface,
    add_ref: dt_add_ref,
    release: dt_release,
    drag_enter: dt_drag_enter,
    drag_over: dt_drag_over,
    drag_leave: dt_drag_leave,
    drop: dt_drop,
};
unsafe fn extract_hdrop_paths(p_data_obj:*mut IDataObject)->Vec<String> {
    if p_data_obj.is_null() { return Vec::new(); }
    let vtbl = (*p_data_obj).lp_vtbl;
    if vtbl.is_null() { return Vec::new(); }
    let format_etc = FormatEtc {
        cf_format: 15,
        _pad: [0; 6],
        ptd: null_mut(),
        dw_aspect: 1,
        lindex: -1,
        tymed: 1,
    };
    let mut medium: StgMedium = std::mem::zeroed();
    let get_data = (*vtbl).get_data;
    if get_data(p_data_obj as *mut c_void,&format_etc,&mut medium) != 0 {
        return Vec::new();
    }
    let mut paths = Vec::new();
    let hdrop = medium.h_global;
    if !hdrop.is_null() {
        let count = DragQueryFileW(hdrop,0xFFFFFFFF,null_mut(),0);
        paths.reserve(count as usize);
        for i in 0..count {
            let len = DragQueryFileW(hdrop,i,null_mut(),0);
            let mut buf = vec![0u16;(len+1) as usize];
            DragQueryFileW(hdrop,i,buf.as_mut_ptr(),len+1);
            buf.pop();
            paths.push(String::from_utf16_lossy(&buf));
        }
    }
    ReleaseStgMedium(&mut medium);
    paths
}
fn wide(s:&str)->Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }
const fn rgb(r:u32,g:u32,b:u32)->u32 { r | g<<8 | b<<16 }
// Logical-pixel geometry shared by painting and hit testing.
// Collapsed: narrow while only the clock shows, widening to COL_W as playback brings in artwork and waveform.
const CLOCK_W:f64=200.; const COL_W:f64=320.; const COL_H:f64=36.;
/// Notification pill: the collapsed notch grows to this size while a notification shows.
const NOTICE_W:f64=380.; const NOTICE_H:f64=68.; const NOTICE_ICON:f64=38.;
/// Minimal notch: what the closed notch shrinks to while it would cover another window's content.
const MINI_W:f64=120.; const MINI_H:f64=6.;
const EXP_W:f64=640.; const EXP_H:f64=196.;
// Each page has its own expanded width; the panel springs between them. The DIB leaves room for overshoot.
const NP_W:f64=460.; const CAL_W:f64=440.; const DIB_W:f64=EXP_W+48.;
const TODO_W:f64=440.; const POMO_W:f64=440.;
const TAB_MUSIC:usize=0; const TAB_CAL:usize=1; const TAB_TODO:usize=2; const TAB_POMO:usize=3; const TAB_SHELF:usize=4; const TAB_SETTINGS:usize=5; const TAB_COUNT:usize=6;
fn page_w(tab:usize)->f64 {[NP_W,CAL_W,TODO_W,POMO_W,EXP_W,SET_W][tab.min(TAB_COUNT-1)]}
/// Settings page: two inset grouped cards of switches, the music-player segmented control, a footer.
const SET_W:f64=440.; const SET_H:f64=300.;
const SET_CARD_Y:f64=64.; const SET_ROW_H:f64=32.; const SET_CARD_W:f64=190.; const SET_SEG_Y:f64=226.; const SET_SEG_H:f64=30.;
/// Switch rows in layout order: four in the "Notch" card, three in "General". Ids are `tray::item`s.
const SETTING_ROWS:[(&str,usize);7]=[
    ("Open on hover",tray::item::OPEN_ON_HOVER),
    ("Shrink over content",tray::item::MINIMAL),
    ("Lyrics when closed",tray::item::COLLAPSED_LYRICS),
    ("Notifications",tray::item::NOTIFICATIONS),
    ("24-hour clock",tray::item::CLOCK_24H),
    ("Match Windows motion",tray::item::FOLLOW_ANIMATIONS),
    ("Start with Windows",tray::item::AUTOSTART),
];
/// Card x and row y (page coordinates) of switch row `k`.
fn setting_row(k:usize)->(f64,f64) {if k<4 {(24.,SET_CARD_Y+k as f64*SET_ROW_H)} else {(226.,SET_CARD_Y+(k-4) as f64*SET_ROW_H)}}
const ROW_Y0:f64=82.; const ROW_H:f64=26.; const TODO_ROWS:usize=4;
const HEADER_H:f64=40.;
const MEDIA_X:f64=24.; const MEDIA_R:f64=NP_W-24.; const ART_SIZE:f64=72.;
// Now Playing is laid out from the panel's bottom edge so it can grow for lyrics: scrubber at h-66, controls at h-26.
const TRANSPORT_X:[f64;3]=[182.,230.,278.];
const NP_LYRICS_H:f64=300.; const LYRIC_ART:f64=44.; const DIB_H:f64=NP_LYRICS_H+40.;
// Collapsed lyric line: the notch widens to fit it, up to MAX_LYRIC_W.
const MAX_LYRIC_W:f64=620.;
const LYRICS_BTN_X:f64=40.; const SPEAKER_X:f64=330.; const VOL_X0:f64=346.; const VOL_X1:f64=436.;
const CARD_W:f64=120.; const CARD_H:f64=118.; const CARD_GAP:f64=12.; const CARD_Y:f64=50.;
const CLOSE_DX:f64=16.; const CLOSE_DY:f64=14.;
// Apple dark-mode label and system colors.
const WHITE:u32=rgb(245,245,247); const SECONDARY:u32=rgb(152,152,158); const TERTIARY:u32=rgb(99,99,104);
const RED:u32=rgb(255,69,58); const BLUE:u32=rgb(10,132,255); const ORANGE:u32=rgb(255,159,10); const GREEN:u32=rgb(48,209,88);
/// Clear / Clear Done sit just left of the Settings tab at the header's right edge.
fn clear_rect(panel_w:f64,tab:usize)->(f64,f64) {if tab==TAB_TODO {(panel_w-154.,90.)} else {(panel_w-124.,60.)}}
fn in_drop_zone(x:f64,y:f64)->bool {(24. ..EXP_W-24.).contains(&x) && (CARD_Y..EXP_H-16.).contains(&y)}
#[derive(Clone,Debug)]
struct ShelfItem { path:String, name:String, ext:String, size_str:String, is_dir:bool, children:Vec<ShelfItem> }
impl ShelfItem {
    fn is_group(&self)->bool { !self.children.is_empty() }
}
#[derive(Clone,Copy,PartialEq,Eq,Debug)]
enum FileKind { Folder, Image, Audio, Code, Archive, Document, Generic }
struct State { expanded:bool, progress:f64, velocity:f64, play_progress:f64, play_velocity:f64, reduced_motion:bool, scale:f64, fonts:[Handle;8], ticks:u32, smoke:bool, verified:bool, smoke_shelf_verified:bool, smoke_drag_verified:bool, drop_target:*mut DropTarget, media:media::Media, receiver:std::sync::mpsc::Receiver<media::Media>, hover:i32, pressed:i32, clock:std::time::Instant, last_frame:std::time::Instant, dc:Handle, bitmap:Handle, original:Handle, pixels:*mut u32, stride:usize, text_buffer:std::cell::RefCell<Vec<u16>>, text_cache:std::cell::RefCell<TextCache>, icon_cache:std::cell::RefCell<IconCache>, renderer:Option<render::Renderer>, running:std::sync::Arc<std::sync::atomic::AtomicBool>, pace:std::sync::Arc<std::sync::atomic::AtomicU8>, menu_active:bool, last_minute:u16, shelf_items:Vec<ShelfItem>, active_tab:usize, tab_from:usize, tab_to:usize, tab_t:f64, tab_vel:f64, panel_w:f64, panel_vel:f64, wheel_acc:f64, todos:Vec<tasks::Todo>, todo_input:String, todo_editing:bool, todo_scroll:usize, high_surrogate:u16, pomo:tasks::Pomodoro, timer_t:f64, timer_vel:f64, alert_until:Option<std::time::Instant>, last_pomo_sec:i64, lyrics_tx:Option<std::sync::mpsc::Sender<lyrics::Request>>, lyrics_rx:Option<std::sync::mpsc::Receiver<lyrics::Reply>>, lyrics_key:String, lyrics:lyrics::Status, lyrics_on:bool, lyrics_t:f64, lyrics_tv:f64, lyric_scroll:f64, lyric_vel:f64, lyric_heights:std::cell::RefCell<Vec<f64>>, lyric_hits:std::cell::RefCell<Vec<(f64,f64,usize)>>, play_pos:f64, panel_h:f64, panel_hv:f64, volume:volume::Volume, vol_level:f32, vol_muted:bool, vol_drag:bool, clyric_t:f64, clyric_tv:f64, lyric_w:f64, lyric_wv:f64, lyric_w_target:f64, clyric_key:(i64,bool), last_sample:f64, dragging_out:bool, shelf_saved:String, tray:Option<tray::Tray>, shelf_scroll:f64, shelf_hover:i32, shelf_remove_hover:i32, tab_hover:i32, add_button_hover:bool, active_folder:Option<usize>, breadcrumb_hover:bool, drag_source:Option<usize>, drag_start_pos:(f64,f64), drag_current_pos:(f64,f64), is_dragging:bool, drag_target:Option<usize>, ole_drag_target:Option<usize>, notify_rx:Option<std::sync::mpsc::Receiver<notify::Notice>>, notices:std::collections::VecDeque<notify::Notice>, notice:Option<notify::Notice>, notice_until:Option<std::time::Instant>, notice_t:f64, notice_v:f64, notice_hover:bool, settings:settings::Settings, autostart:bool, switch_t:[f64;7], switch_v:[f64;7], seg_t:f64, seg_v:f64, mini_t:f64, mini_v:f64, covered:bool, cover_pending:bool, cover_count:u8, cover_checked:std::time::Instant, cover_switched:std::time::Instant, cover_w:f64 }
impl State {
    unsafe fn new(hwnd:Hwnd,smoke:bool,drop_target:*mut DropTarget)->Self {
        let scale=GetDpiForWindow(hwnd) as f64/96.;
        // SF Pro Display is Apple's and can't be shipped with Wisp, so it is used only when installed.
        // Otherwise Wisp uses Inter, which is bundled (SIL Open Font License) and loaded privately.
        let face_for=font_faces();
        let fonts=[(15,500),(14,400),(12,400),(11,500),(40,400),(30,500),(20,500),(13,500)].map(|(n,weight)| {
            let face=wide(face_for(weight));
            CreateFontW(-(n as f64*scale) as i32,0,0,0,weight,0,0,0,1,0,0,4,0,face.as_ptr())
        });
        let dc=CreateCompatibleDC(null_mut());
        let stride=(DIB_W*scale).ceil() as usize;
        let info=BitmapInfo{size:40,width:stride as i32,height:-(DIB_H*scale).ceil() as i32,planes:1,bits:32,compression:0,bytes:0,x:0,y:0,used:0,important:0};
        let mut pixels=null_mut();let bitmap=CreateDIBSection(dc,&info,0,&mut pixels,null_mut(),0);let original=SelectObject(dc,bitmap);
        let renderer=Some(render::Renderer::new(pixels.cast(),stride,-info.height,scale));
        let mut now=Time::default();GetLocalTime(&mut now);
        let initial_media_playing=false;
        let (lyrics_tx,lyrics_rx)=if smoke {(None,None)} else {let (tx,rx)=lyrics::start();(Some(tx),Some(rx))};
        let volume=volume::Volume::none();
        let shelf_items=if smoke {Vec::new()} else {load_shelf()};
        let shelf_saved=serialize_shelf(&shelf_items);
        let (vol_level,vol_muted)=volume.get().unwrap_or((-1.,false));
        Self {expanded:false,progress:0.,velocity:0.,play_progress:if initial_media_playing {1.}else{0.},play_velocity:0.,reduced_motion:reduced_motion(),scale,fonts,ticks:0,smoke,verified:false,smoke_shelf_verified:false,smoke_drag_verified:false,drop_target,media:media::Media{title:"Not playing".into(),artist:"Play music on Spotify".into(),..Default::default()},receiver:media::start(),hover:-1,pressed:-1,clock:std::time::Instant::now(),last_frame:std::time::Instant::now(),dc,bitmap,original,pixels:pixels.cast(),stride,text_buffer:std::cell::RefCell::new(Vec::with_capacity(256)),text_cache:Default::default(),icon_cache:Default::default(),renderer,running:std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),pace:std::sync::Arc::new(std::sync::atomic::AtomicU8::new(PACE_MOTION)),menu_active:false,last_minute:now.minute,shelf_items,active_tab:0,tab_from:TAB_MUSIC,tab_to:TAB_MUSIC,tab_t:1.,tab_vel:0.,panel_w:NP_W,panel_vel:0.,wheel_acc:0.,todos:if smoke {Vec::new()} else {tasks::load_todos()},todo_input:String::new(),todo_editing:false,todo_scroll:0,high_surrogate:0,pomo:tasks::Pomodoro::new(),timer_t:0.,timer_vel:0.,alert_until:None,last_pomo_sec:0,lyrics_tx,lyrics_rx,lyrics_key:String::new(),lyrics:lyrics::Status::NotFound,lyrics_on:false,lyrics_t:0.,lyrics_tv:0.,lyric_scroll:-1.,lyric_vel:0.,lyric_heights:std::cell::RefCell::new(Vec::new()),lyric_hits:std::cell::RefCell::new(Vec::new()),play_pos:0.,panel_h:EXP_H,panel_hv:0.,volume,vol_level,vol_muted,vol_drag:false,clyric_t:0.,clyric_tv:0.,lyric_w:COL_W,lyric_wv:0.,lyric_w_target:COL_W,clyric_key:(-2,false),last_sample:-1.,dragging_out:false,shelf_saved,tray:if smoke {None} else {Some(tray::Tray::new(hwnd))},shelf_scroll:0.,shelf_hover:-1,shelf_remove_hover:-1,tab_hover:-1,add_button_hover:false,active_folder:None,breadcrumb_hover:false,drag_source:None,drag_start_pos:(0.,0.),drag_current_pos:(0.,0.),is_dragging:false,drag_target:None,ole_drag_target:None,notify_rx:if smoke {None} else {Some(notify::start())},notices:std::collections::VecDeque::new(),notice:None,notice_until:None,notice_t:0.,notice_v:0.,notice_hover:false,autostart:!smoke && settings::starts_with_windows(),switch_t:[0.;7],switch_v:[0.;7],seg_t:0.,seg_v:0.,mini_t:0.,mini_v:0.,covered:false,cover_pending:false,cover_count:0,cover_checked:std::time::Instant::now(),cover_switched:std::time::Instant::now(),cover_w:0.,settings:if smoke {settings::Settings::default()} else {settings::Settings::load()}}
    }
    fn px(&self,n:f64)->i32 {(n*self.scale).round() as i32}
    fn reveal(&self)->f64 {let t=((self.progress-0.12)/0.88).clamp(0.,1.);t*t*(3.-2.*t)}
    fn offset(&self)->(f64,f64) {(0.,-18.*(1.-self.reveal()))}
    /// Whether page content takes input. Interaction starts once the panel is essentially in place
    /// (within ~1.5 pt and nearly opaque) rather than waiting out every spring's last fraction, which
    /// left quick grabs ignored for about half a second.
    fn usable(&self)->bool {
        self.expanded && self.progress>0.94 && self.tab_to==self.active_tab && self.tab_t>0.92
            && (self.panel_w-page_w(self.active_tab)).abs()<1.5 && (self.panel_h-self.page_h(self.active_tab)).abs()<1.5
    }
    /// Expanded height of a page: Now Playing grows while lyrics are shown.
    fn page_h(&self,tab:usize)->f64 {if tab==TAB_MUSIC && self.lyrics_on {NP_LYRICS_H} else if tab==TAB_SETTINGS {SET_H} else {EXP_H}}
    /// The collapsed notch shows the sung line while lyrics are switched on and a synced line is current.
    fn collapsed_lyrics_active(&self)->bool {
        self.settings.collapsed_lyrics && self.lyrics_on && self.media.playing && matches!(self.lyrics,lyrics::Status::Synced(_)) && self.lyric_target()>=0.
    }
    fn panel_h_now(&self)->f64 {let c=self.col_h_now();(c+(self.panel_h-c)*self.progress).min(DIB_H)}
    /// Collapsed height: the 36 pt strip, or the notification pill while one shows.
    fn col_h_now(&self)->f64 {let mini=COL_H+(MINI_H-COL_H)*self.mini_t;mini+(NOTICE_H-mini)*self.notice_t.max(0.)}
    /// A notification is on screen (or arriving); hover and clicks go to it instead of expanding.
    fn notice_shown(&self)->bool {self.notice_until.is_some() && !self.expanded}
    /// Opacity of the regular collapsed content, which makes way for a notification or minimal mode.
    fn collapsed_fade(&self)->f64 {(1.-self.notice_t.clamp(0.,1.)*2.5).clamp(0.,1.)*(1.-self.mini_t.clamp(0.,1.)*2.5).clamp(0.,1.)}
    /// Checks a few times a second whether the closed notch would cover another window (say, a maximized
    /// browser's tabs). A change must be seen twice in a row, so windows passing by don't make it flicker.
    unsafe fn update_cover(&mut self,hwnd:Hwnd,now:std::time::Instant) {
        if self.smoke || !self.settings.minimal_when_covering {self.covered=false;return;}
        if now.duration_since(self.cover_checked)<std::time::Duration::from_millis(250) {return;}
        self.cover_checked=now;
        // While shrunk, check the widest the notch has been since (lyric lines change its width), so a
        // shorter line can't make the notch pop back out.
        if self.covered {self.cover_w=self.cover_w.max(self.col_base_w());}
        let w=self.px(if self.covered {self.cover_w} else {self.col_base_w()});let screen=GetSystemMetrics(0);
        let footprint=Rect{left:(screen-w)/2,top:0,right:(screen+w)/2,bottom:self.px(COL_H)};
        // A window behind the notch only counts if something is actually drawn there: an empty title bar
        // or tab strip is one flat color, while tabs, text or icons reaching the notch are not. While the
        // notch is normal only thin strips just beside it are visible; once it is a sliver, the area under
        // it shows too, so it grows back only when that whole area is clear again.
        if self.expanded || self.notice.is_some() {return;}
        let covers=covers_window(hwnd,footprint) && {
            let (gap,strip,bottom)=(self.px(2.),self.px(14.),footprint.bottom);
            let mut areas=vec![
                Rect{left:footprint.left-gap-strip,top:0,right:footprint.left-gap,bottom},
                Rect{left:footprint.right+gap,top:0,right:footprint.right+gap+strip,bottom},
            ];
            if self.covered && self.mini_t>0.99 {
                let mini=self.px(MINI_W)/2+self.px(8.);
                areas.push(Rect{left:footprint.left,top:0,right:screen/2-mini,bottom});
                areas.push(Rect{left:screen/2+mini,top:0,right:footprint.right,bottom});
                areas.push(Rect{left:screen/2-mini,top:self.px(MINI_H)+gap,right:screen/2+mini,bottom});
            }
            !screen_is_flat(&areas)
        };
        let since=now.duration_since(self.cover_switched).as_secs_f64();
        let next=cover_step(self.covered,&mut self.cover_pending,&mut self.cover_count,since,covers);
        if next!=self.covered {
            self.covered=next;self.cover_switched=now;
            if next {self.cover_w=self.col_base_w();}
            InvalidateRect(hwnd,std::ptr::null(),0);
        }
    }
    /// Index of the lyric line being sung (fractional targets are not used), -1 before the first line.
    fn lyric_target(&self)->f64 {
        match &self.lyrics {
            lyrics::Status::Synced(lines)=>{let at=self.play_pos+0.25;lines.partition_point(|l|l.time<=at) as f64-1.}
            _=>-1.,
        }
    }
    /// Asks LRCLIB again for the current track, skipping the disk cache.
    fn retry_lyrics(&mut self) {
        self.lyrics_key.clear();
        self.fetch_lyrics(true);
    }
    /// Asks LRCLIB for the current track's lyrics once per track.
    fn request_lyrics(&mut self) {self.fetch_lyrics(false);}
    fn fetch_lyrics(&mut self,fresh:bool) {
        let playing_something=!(self.media.title==media::IDLE_TITLE && self.media.artist==media::IDLE_ARTIST) && !self.media.title.is_empty();
        let key=if playing_something {format!("{}\0{}",self.media.title,self.media.artist)} else {String::new()};
        if key==self.lyrics_key {return;}
        self.lyrics_key=key;
        self.lyric_scroll=-1.;self.lyric_vel=0.;
        self.lyric_heights.borrow_mut().clear();self.lyric_hits.borrow_mut().clear();
        self.lyrics=lyrics::Status::NotFound;
        if !playing_something {return;}
        if let Some(tx)=&self.lyrics_tx {
            let request=lyrics::Request{key:self.lyrics_key.clone(),title:self.media.title.clone(),artist:self.media.artist.clone(),album:self.media.album.clone(),duration:self.media.duration,fresh};
            if tx.send(request).is_ok() {self.lyrics=lyrics::Status::Loading;}
        }
    }
    /// The lyrics column shows a "no lyrics" or "couldn't reach" message that can be retried.
    fn lyrics_retryable(&self)->bool {
        !self.lyrics_key.is_empty() && matches!(self.lyrics,lyrics::Status::NotFound|lyrics::Status::Failed)
    }
    /// Now Playing targets: 0..=2 transport, 4 retry lyrics, 5 lyrics, 6 mute, 7 volume slider, 8 scrubber, 9000+i lyric line.
    fn music_hit(&self,x:f64,y:f64)->i32 {
        let cy=self.panel_h-26.;
        for (i,&cx) in TRANSPORT_X.iter().enumerate() {let (dx,dy)=(x-cx,y-cy);if dx*dx+dy*dy<=20.*20. {return i as i32;}}
        let (dx,dy)=(x-LYRICS_BTN_X,y-cy);
        if dx*dx+dy*dy<=17.*17. {return 5;}
        if self.vol_level>=0. {
            if (x-SPEAKER_X).abs()<=13. && (y-cy).abs()<=13. {return 6;}
            if (VOL_X0-6. ..VOL_X1+6.).contains(&x) && (y-cy).abs()<=11. {return 7;}
        }
        if (self.panel_h-76. ..self.panel_h-54.).contains(&y) && (MEDIA_X..MEDIA_R).contains(&x) {return 8;}
        if self.lyrics_t==1. && self.lyrics_retryable() && (MEDIA_X..MEDIA_R).contains(&x) && (100. ..self.panel_h-84.).contains(&y) {return 4;}
        if self.lyrics_t==1. && (MEDIA_X-4. ..MEDIA_R).contains(&x) {
            for &(y0,y1,i) in self.lyric_hits.borrow().iter() {if (y0..y1).contains(&y) {return 9000+i as i32;}}
        }
        -1
    }
    fn set_volume_x(&mut self,x:f64) {
        let level=((x-VOL_X0)/(VOL_X1-VOL_X0)).clamp(0.,1.) as f32;
        self.volume.set_level(level);self.vol_level=level;
        if level>0. {self.vol_muted=false;}
    }
    fn music_click(&mut self,x:f64,hit:i32) {
        match hit {
            0=>media::prev(),
            1=>media::play_pause(),
            2=>media::next(),
            4=>self.retry_lyrics(),
            5=>{
                self.lyrics_on=!self.lyrics_on;
                // Turning lyrics on after a failed lookup asks LRCLIB again.
                if self.lyrics_on && matches!(self.lyrics,lyrics::Status::Failed) {self.retry_lyrics();}
            }
            6=>{self.vol_muted=!self.vol_muted;self.volume.set_muted(self.vol_muted);}
            8 if self.media.duration>0=>{
                let t=((x-MEDIA_X)/(MEDIA_R-MEDIA_X)).clamp(0.,1.)*self.media.duration as f64;
                media::seek(t as i64);self.play_pos=t.floor();
            }
            9000..=99999=>{
                if let lyrics::Status::Synced(lines)=&self.lyrics {
                    if let Some(line)=lines.get((hit-9000) as usize) {let t=line.time.ceil();media::seek(t as i64);self.play_pos=t;}
                }
            }
            _=>{}
        }
    }
    /// Current window width in logical pixels.
    /// The closed notch's normal width (clock, music, timer, lyric line), before minimal mode or a notification.
    fn col_base_w(&self)->f64 {CLOCK_W+(COL_W-CLOCK_W)*self.play_progress.max(self.timer_t)+(self.lyric_w-COL_W).max(0.)*self.clyric_t}
    fn col_w_now(&self)->f64 {
        let base=self.col_base_w();
        let mini=base+(MINI_W-base)*self.mini_t;
        mini+(NOTICE_W-mini)*self.notice_t.max(0.)
    }
    fn panel_w_now(&self)->f64 {let c=self.col_w_now();(c+(self.panel_w-c)*self.progress).min(DIB_W)}
    /// Opacity and horizontal slide for a page during a page switch. The outgoing page fades out
    /// early while the incoming one fades in late, so the two never read as a double exposure.
    fn page_mix(&self,page:usize)->(f64,f64) {
        let ss=|a:f64,b:f64,x:f64|{let k=((x-a)/(b-a)).clamp(0.,1.);k*k*(3.-2.*k)};
        let dir=if self.tab_to>=self.tab_from {1.} else {-1.};
        let t=self.tab_t;
        if page==self.tab_to {(ss(0.3,1.,t),dir*28.*(1.-t))}
        else if page==self.tab_from {(1.-ss(0.,0.35,t),-dir*28.*t)}
        else {(0.,0.)}
    }
    fn shelf_tab_w(&self)->f64 {let n=self.total_items();if n==0 {34.} else if n>=10 {58.} else {52.}}
    /// Header tab positions: the five pages from the left, Settings alone at the right edge.
    fn tab_x(&self,tab:usize)->(f64,f64) {if tab<TAB_SHELF {(26.+38.*tab as f64,34.)} else if tab==TAB_SHELF {(178.,self.shelf_tab_w())} else {(self.panel_w_now()-26.-34.,34.)}}
    /// Right edge of the header tabs.
    fn tabs_end(&self)->f64 {178.+self.shelf_tab_w()}
    /// Where the shelf's folder breadcrumb starts: after the shelf tab.
    fn breadcrumb_x(&self)->f64 {self.tabs_end()+16.}
    fn has_clear(&self,tab:usize)->bool {
        match tab {TAB_SHELF=>self.total_items()>0,TAB_TODO=>self.todos.iter().any(|t|t.done),_=>false}
    }
    /// Header targets: 0..TAB_COUNT page tabs, 10 Clear, 12 folder breadcrumb, -1 none.
    fn header_hit(&self,x:f64,y:f64)->i32 {
        if !(8. ..36.).contains(&y) {return -1;}
        if self.active_tab==TAB_SHELF && self.active_folder.is_some() {let bx=self.breadcrumb_x();if (bx..bx+130.).contains(&x) {return 12;}}
        for tab in 0..TAB_COUNT {let (tx,tw)=self.tab_x(tab);if (tx..tx+tw).contains(&x) {return tab as i32;}}
        if self.has_clear(self.active_tab) {let (cx,cw)=clear_rect(self.panel_w_now(),self.active_tab);if (cx..cx+cw).contains(&x) {return 10;}}
        -1
    }
    /// To-do targets: 1000 input field, 2000+i row, 3000+i row delete, -1 none.
    fn todo_hit(&self,x:f64,y:f64)->i32 {
        if !(24. ..TODO_W-24.).contains(&x) {return -1;}
        if (44. ..74.).contains(&y) {return 1000;}
        if y<ROW_Y0 {return -1;}
        let k=((y-ROW_Y0)/ROW_H).floor() as usize;
        let i=self.todo_scroll+k;
        if k>=TODO_ROWS || i>=self.todos.len() {return -1;}
        if (388. ..412.).contains(&x) {3000+i as i32} else {2000+i as i32}
    }
    fn save_todos(&self) {if !self.smoke {tasks::save_todos(&self.todos);}}
    fn clamp_todo_scroll(&mut self) {self.todo_scroll=self.todo_scroll.min(self.todos.len().saturating_sub(TODO_ROWS));}
    fn push_input(&mut self,ch:char) {if self.todo_input.chars().count()<120 {self.todo_input.push(ch);}}
    fn commit_todo(&mut self) {
        let text=self.todo_input.trim().to_string();
        if text.is_empty() {return;}
        self.todos.push(tasks::Todo{text,done:false});
        self.todo_input.clear();
        self.todo_scroll=self.todos.len().saturating_sub(TODO_ROWS);
        self.save_todos();
    }
    unsafe fn todo_click(&mut self,hwnd:Hwnd,hit:i32) {
        match hit {
            1000=>{self.todo_editing=true;SetForegroundWindow(hwnd);SetFocus(hwnd);}
            2000..=2999=>{
                let i=(hit-2000) as usize;
                if let Some(t)=self.todos.get_mut(i) {t.done=!t.done;}
                self.todo_editing=false;self.save_todos();
            }
            3000..=3999=>{
                let i=(hit-3000) as usize;
                if i<self.todos.len() {self.todos.remove(i);}
                self.clamp_todo_scroll();self.hover=-1;self.todo_editing=false;self.save_todos();
            }
            _=>{self.todo_editing=false;}
        }
    }
    fn pomo_click(&mut self,hit:i32) {
        match hit {
            4000..=4002=>self.pomo.select(tasks::Phase::ALL[(hit-4000) as usize]),
            4010=>self.pomo.reset(),
            4011=>self.pomo.toggle(std::time::Instant::now()),
            4020|4021=>{
                if self.pomo.adjust(if hit==4020 {-1} else {1}) {
                    self.settings.pomo_minutes=self.pomo.all_minutes();
                    if !self.smoke {self.settings.save();}
                }
            }
            _=>{}
        }
    }
    fn advance(&mut self,dt:f64)->bool {
        let geom_target=if self.expanded {1.}else{0.};
        let geom_animating=if self.progress==geom_target && self.velocity==0. {
            false
        } else if self.reduced_motion {
            self.progress=geom_target;self.velocity=0.;true
        } else {
            let omega=22.;let displacement=self.progress-geom_target;
            let c=self.velocity+omega*displacement;let decay=(-omega*dt).exp();
            self.progress=(geom_target+(displacement+c*dt)*decay).clamp(0.,1.);
            self.velocity=(self.velocity-omega*c*dt)*decay;
            if (self.progress-geom_target).abs()<0.0001 && self.velocity.abs()<0.002 {self.progress=geom_target;self.velocity=0.;}
            true
        };
        let play_target=if self.media.playing {1.}else{0.};
        let play_animating=if self.play_progress==play_target && self.play_velocity==0. {
            false
        } else if self.reduced_motion {
            self.play_progress=play_target;self.play_velocity=0.;true
        } else {
            let omega=22.;let displacement=self.play_progress-play_target;
            let c=self.play_velocity+omega*displacement;let decay=(-omega*dt).exp();
            self.play_progress=(play_target+(displacement+c*dt)*decay).clamp(0.,1.);
            self.play_velocity=(self.play_velocity-omega*c*dt)*decay;
            if (self.play_progress-play_target).abs()<0.0001 && self.play_velocity.abs()<0.002 {self.play_progress=play_target;self.play_velocity=0.;}
            true
        };
        if self.active_tab!=self.tab_to {
            if self.active_tab==self.tab_from && self.tab_t<1. {
                // Reversing mid-switch keeps the current position and momentum.
                self.tab_from=self.tab_to;self.tab_t=(1.-self.tab_t).clamp(0.,1.);self.tab_vel=-self.tab_vel;
            } else {
                if self.tab_t<0.5 {self.tab_to=self.tab_from;}
                self.tab_from=self.tab_to;self.tab_t=0.;self.tab_vel=0.;
            }
            self.tab_to=self.active_tab;
        }
        let hidden=!self.expanded && self.progress==0.;
        let tab_animating=if self.reduced_motion || hidden {
            let moving=self.tab_t!=1. || self.tab_vel!=0.;self.tab_t=1.;self.tab_vel=0.;moving
        } else {spring(&mut self.tab_t,&mut self.tab_vel,1.,19.,0.86,dt)};
        let width_target=page_w(self.active_tab);
        let width_animating=if self.reduced_motion || hidden {
            let moving=self.panel_w!=width_target || self.panel_vel!=0.;self.panel_w=width_target;self.panel_vel=0.;moving
        } else {spring(&mut self.panel_w,&mut self.panel_vel,width_target,19.,0.78,dt)};
        let height_target=self.page_h(self.active_tab);
        let height_animating=if self.reduced_motion || hidden {
            let moving=self.panel_h!=height_target || self.panel_hv!=0.;self.panel_h=height_target;self.panel_hv=0.;moving
        } else {spring(&mut self.panel_h,&mut self.panel_hv,height_target,19.,0.78,dt)};
        let lyrics_target=if self.lyrics_on {1.} else {0.};
        let lyrics_animating=if self.reduced_motion || hidden {
            let moving=self.lyrics_t!=lyrics_target || self.lyrics_tv!=0.;self.lyrics_t=lyrics_target;self.lyrics_tv=0.;moving
        } else {spring(&mut self.lyrics_t,&mut self.lyrics_tv,lyrics_target,19.,0.9,dt)};
        if self.media.playing {
            self.play_pos+=dt;
            if self.media.duration>0 {self.play_pos=self.play_pos.min(self.media.duration as f64);}
        }
        // Lyrics glide to the sung line on a soft spring, like Apple Music.
        let line_target=self.lyric_target();
        let scroll_animating=if self.reduced_motion {
            let moving=self.lyric_scroll!=line_target;self.lyric_scroll=line_target;self.lyric_vel=0.;moving
        } else {spring(&mut self.lyric_scroll,&mut self.lyric_vel,line_target,11.,0.92,dt)};
        if self.active_tab!=TAB_TODO {self.todo_editing=false;}
        let timer_target=if self.pomo.active() {1.} else {0.};
        let timer_animating=if self.reduced_motion {
            let moving=self.timer_t!=timer_target || self.timer_vel!=0.;self.timer_t=timer_target;self.timer_vel=0.;moving
        } else {spring(&mut self.timer_t,&mut self.timer_vel,timer_target,22.,1.,dt)};
        let active=self.collapsed_lyrics_active();
        let clyric_target=if active {1.} else {0.};
        let lyric_w_target=if active {self.lyric_w_target} else {COL_W};
        let clyric_animating=if self.reduced_motion {
            let moving=self.clyric_t!=clyric_target || self.lyric_w!=lyric_w_target;
            self.clyric_t=clyric_target;self.clyric_tv=0.;self.lyric_w=lyric_w_target;self.lyric_wv=0.;moving
        } else {
            let a=spring(&mut self.clyric_t,&mut self.clyric_tv,clyric_target,19.,0.9,dt);
            let b=spring(&mut self.lyric_w,&mut self.lyric_wv,lyric_w_target,16.,0.8,dt);
            a || b
        };
        // Settings switches and the segmented control's thumb glide to their values.
        let mut settings_animating=false;
        for (k,&(_,item)) in SETTING_ROWS.iter().enumerate() {
            let target=if self.setting_on(item) {1.} else {0.};
            if self.reduced_motion {settings_animating|=self.switch_t[k]!=target;self.switch_t[k]=target;self.switch_v[k]=0.;}
            else {settings_animating|=spring(&mut self.switch_t[k],&mut self.switch_v[k],target,24.,0.82,dt);}
        }
        let seg_target=settings::Player::ALL.iter().position(|p|*p==self.settings.player).unwrap_or(0) as f64;
        if self.reduced_motion {settings_animating|=self.seg_t!=seg_target;self.seg_t=seg_target;self.seg_v=0.;}
        else {settings_animating|=spring(&mut self.seg_t,&mut self.seg_v,seg_target,20.,0.86,dt);}
        // Minimal mode: the closed notch shrinks to a sliver at the top edge while it would cover a window.
        let mini_target=if self.covered && !self.expanded {1.} else {0.};
        let mini_animating=if self.reduced_motion {
            let moving=self.mini_t!=mini_target || self.mini_v!=0.;self.mini_t=mini_target;self.mini_v=0.;moving
        } else {spring(&mut self.mini_t,&mut self.mini_v,mini_target,19.,0.86,dt)};
        // The notification pill pops open with a slight overshoot, like the Dynamic Island.
        let notice_target=if self.notice_until.is_some() {1.} else {0.};
        let notice_animating=if self.reduced_motion {
            let moving=self.notice_t!=notice_target || self.notice_v!=0.;self.notice_t=notice_target;self.notice_v=0.;moving
        } else {spring(&mut self.notice_t,&mut self.notice_v,notice_target,17.,0.72,dt)};
        geom_animating || play_animating || tab_animating || width_animating || timer_animating || height_animating || lyrics_animating || scroll_animating || clyric_animating || notice_animating || mini_animating || settings_animating
    }
    /// Pulls new notifications, starts the next one when the notch is free, and retires finished ones.
    unsafe fn update_notices(&mut self,hwnd:Hwnd,now:std::time::Instant) {
        if let Some(rx)=&self.notify_rx {
            for notice in rx.try_iter() {
                if !self.settings.notifications {continue;}
                self.notices.push_back(notice);
                if self.notices.len()>4 {self.notices.pop_front();}
            }
        }
        if let Some(until)=self.notice_until {
            if self.expanded || (now>=until && !self.notice_hover) {self.notice_until=None;InvalidateRect(hwnd,std::ptr::null(),0);}
        }
        if self.notice.is_some() && self.notice_until.is_none() && self.notice_t.abs()<0.01 && self.notice_v.abs()<0.05 {
            self.notice=None;self.notice_t=0.;self.notice_v=0.;
            if let Some(r)=self.renderer.as_mut() {r.set_icon(None);}
            self.layout(hwnd);
        }
        if self.notice.is_none() && !self.expanded && self.progress==0. {
            if let Some(notice)=self.notices.pop_front() {
                if let Some(r)=self.renderer.as_mut() {r.set_icon(notice.icon.as_deref());}
                // Shorter when more are waiting, so a burst does not queue up for long.
                let secs=if self.notices.is_empty() {5.} else {3.};
                self.notice=Some(notice);self.notice_until=Some(now+std::time::Duration::from_secs_f64(secs));
                InvalidateRect(hwnd,std::ptr::null(),0);
            }
        }
    }
    /// Whether the switch for `item` (a `tray::item` id) is on.
    fn setting_on(&self,item:usize)->bool {
        use tray::item::*;
        match item {
            OPEN_ON_HOVER=>self.settings.open_on_hover,
            MINIMAL=>self.settings.minimal_when_covering,
            COLLAPSED_LYRICS=>self.settings.collapsed_lyrics,
            NOTIFICATIONS=>self.settings.notifications,
            CLOCK_24H=>self.settings.clock_24h,
            FOLLOW_ANIMATIONS=>self.settings.follow_windows_animations,
            AUTOSTART=>self.autostart,
            _=>false,
        }
    }
    /// Puts the switches and segmented control at their values without animating (at start-up).
    fn snap_switches(&mut self) {
        for (k,&(_,item)) in SETTING_ROWS.iter().enumerate() {self.switch_t[k]=if self.setting_on(item) {1.} else {0.};}
        self.seg_t=settings::Player::ALL.iter().position(|p|*p==self.settings.player).unwrap_or(0) as f64;
    }
    /// Flips a switch or picks a music player, then applies and saves.
    unsafe fn change_setting(&mut self,hwnd:Hwnd,item:usize) {
        use tray::item::*;
        match item {
            AUTOSTART=>{settings::set_start_with_windows(!self.autostart);self.autostart=settings::starts_with_windows();}
            NOTIFICATIONS=>self.settings.notifications=!self.settings.notifications,
            OPEN_ON_HOVER=>self.settings.open_on_hover=!self.settings.open_on_hover,
            COLLAPSED_LYRICS=>self.settings.collapsed_lyrics=!self.settings.collapsed_lyrics,
            CLOCK_24H=>self.settings.clock_24h=!self.settings.clock_24h,
            FOLLOW_ANIMATIONS=>self.settings.follow_windows_animations=!self.settings.follow_windows_animations,
            MINIMAL=>self.settings.minimal_when_covering=!self.settings.minimal_when_covering,
            i if (PLAYER..PLAYER+settings::Player::ALL.len()).contains(&i)=>self.settings.player=settings::Player::ALL[i-PLAYER],
            _=>return,
        }
        self.apply_settings();
        if !self.smoke {self.settings.save();}
        if !self.settings.notifications {self.notices.clear();self.notice_until=None;}
        InvalidateRect(hwnd,std::ptr::null(),0);
    }
    /// Settings page targets: 5000+k switch row, 5100+i music player, 5200 Quit, -1 none.
    fn settings_hit(&self,x:f64,y:f64)->i32 {
        for k in 0..SETTING_ROWS.len() {
            let (cx,ry)=setting_row(k);
            if (cx..cx+SET_CARD_W).contains(&x) && (ry..ry+SET_ROW_H).contains(&y) {return 5000+k as i32;}
        }
        if (SET_SEG_Y..SET_SEG_Y+SET_SEG_H).contains(&y) && (24. ..SET_W-24.).contains(&x) {return 5100+((x-24.)/((SET_W-48.)/4.)).floor().min(3.) as i32;}
        if (266. ..292.).contains(&y) && (SET_W-24.-92. ..SET_W-24.).contains(&x) {return 5200;}
        -1
    }
    unsafe fn settings_click(&mut self,hwnd:Hwnd,hit:i32) {
        match hit {
            5000..=5006=>self.change_setting(hwnd,SETTING_ROWS[(hit-5000) as usize].1),
            5100..=5103=>self.change_setting(hwnd,tray::item::PLAYER+(hit-5100) as usize),
            5200=>{DestroyWindow(hwnd);}
            _=>{}
        }
    }
    /// Settings that take effect outside the drawing code: motion and which music app is followed.
    fn apply_settings(&mut self) {
        self.reduced_motion=self.settings.follow_windows_animations && unsafe {reduced_motion()};
        media::set_player(self.settings.player.key());
        self.pomo.set_minutes(self.settings.pomo_minutes);
    }
    /// Opens the app that sent the notification on screen, then dismisses it.
    unsafe fn open_notice(&mut self,hwnd:Hwnd) {
        if let Some(notice)=&self.notice {
            let target=format!("shell:AppsFolder\\{}",notice.aumid);
            ShellExecuteW(hwnd,wide("open").as_ptr(),wide("explorer.exe").as_ptr(),wide(&target).as_ptr(),std::ptr::null(),1);
        }
        self.notice_until=None;self.notice_hover=false;
        InvalidateRect(hwnd,std::ptr::null(),0);
    }
    /// Files carried by the card at `idx` in the current view (a group carries all its files).
    fn drag_paths(&self,idx:usize)->Vec<String> {
        match self.current_items().get(idx) {
            Some(item) if item.is_group()=>item.children.iter().map(|c|c.path.clone()).collect(),
            Some(item)=>vec![item.path.clone()],
            None=>Vec::new(),
        }
    }
    fn total_items(&self)->usize {
        self.shelf_items.iter().map(|item| if item.is_group() { item.children.len() } else { 1 }).sum()
    }
    fn current_items(&self)->&[ShelfItem] {
        if let Some(idx)=self.active_folder {
            if idx<self.shelf_items.len() && self.shelf_items[idx].is_group() {
                return &self.shelf_items[idx].children;
            }
        }
        &self.shelf_items
    }
    fn find_card_at_client_pos(&self,x:f64,y:f64)->Option<usize> {
        if self.shelf_items.is_empty() || self.active_tab!=TAB_SHELF { return None; }
        if y<CARD_Y || y>CARD_Y+CARD_H { return None; }
        let count=self.shelf_items.len();
        let card_w=CARD_W; let card_spacing=CARD_GAP;
        let total_w=count as f64*card_w+(count.saturating_sub(1)) as f64*card_spacing;
        let start_x=if count<=4 {(EXP_W-total_w)/2.} else {24.-self.shelf_scroll};
        for idx in 0..count {
            let card_x=start_x+idx as f64*(card_w+card_spacing);
            if x>=card_x && x<=card_x+card_w {
                return Some(idx);
            }
        }
        None
    }
    fn update_ole_drag_hover(&mut self,x:f64,y:f64) {
        if self.active_folder.is_none() {
            self.ole_drag_target=self.find_card_at_client_pos(x,y);
        } else {
            self.ole_drag_target=None;
        }
    }
    fn merge_items(&mut self,source_idx:usize,target_idx:usize) {
        if source_idx==target_idx || source_idx>=self.shelf_items.len() || target_idx>=self.shelf_items.len() {
            return;
        }
        let source_item=self.shelf_items.remove(source_idx);
        let adj_target=if source_idx<target_idx { target_idx-1 } else { target_idx };
        if self.shelf_items[adj_target].is_group() {
            if source_item.is_group() {
                self.shelf_items[adj_target].children.extend(source_item.children);
            } else {
                self.shelf_items[adj_target].children.push(source_item);
            }
            let count=self.shelf_items[adj_target].children.len();
            self.shelf_items[adj_target].size_str=format!("{} items",count);
        } else {
            let target_item=self.shelf_items.remove(adj_target);
            let folder=if source_item.is_group() {
                let mut f=source_item;
                f.children.insert(0,target_item);
                let count=f.children.len();
                f.size_str=format!("{} items",count);
                f
            } else {
                create_folder_item(vec![target_item,source_item])
            };
            self.shelf_items.insert(adj_target,folder);
        }
        if self.shelf_items.len()<=4 { self.shelf_scroll=0.; }
        self.shelf_hover=-1;
        self.shelf_remove_hover=-1;
    }
    fn remove_folder_child(&mut self,folder_idx:usize,child_idx:usize) {
        if folder_idx>=self.shelf_items.len() || !self.shelf_items[folder_idx].is_group() { return; }
        if child_idx>=self.shelf_items[folder_idx].children.len() { return; }
        self.shelf_items[folder_idx].children.remove(child_idx);
        let remaining=self.shelf_items[folder_idx].children.len();
        if remaining==0 {
            self.shelf_items.remove(folder_idx);
            self.active_folder=None;
            self.shelf_scroll=0.;
        } else if remaining==1 {
            let single=self.shelf_items[folder_idx].children.pop().unwrap();
            self.shelf_items[folder_idx]=single;
            self.active_folder=None;
            self.shelf_scroll=0.;
        } else {
            self.shelf_items[folder_idx].size_str=format!("{} items",remaining);
        }
        self.shelf_hover=-1;
        self.shelf_remove_hover=-1;
    }
    fn add_shelf_items(&mut self,paths:&[String],hwnd:Hwnd) {
        let mut added=false;
        if let Some(folder_idx)=self.active_folder {
            if folder_idx<self.shelf_items.len() && self.shelf_items[folder_idx].is_group() {
                for path in paths {
                    let trimmed=path.trim();
                    if trimmed.is_empty() {continue;}
                    if !self.shelf_items[folder_idx].children.iter().any(|item| item.path.eq_ignore_ascii_case(trimmed)) {
                        self.shelf_items[folder_idx].children.push(create_shelf_item(trimmed));
                        added=true;
                    }
                }
                let count=self.shelf_items[folder_idx].children.len();
                self.shelf_items[folder_idx].size_str=format!("{} items",count);
            }
        } else {
            let mut pt=Point::default();
            unsafe { GetCursorPos(&mut pt); }
            let mut r=Rect::default();
            unsafe { GetWindowRect(hwnd,&mut r); }
            let x=(pt.x-r.left) as f64/self.scale;
            let y=(pt.y-r.top) as f64/self.scale;
            let target_card=self.find_card_at_client_pos(x,y);

            if let Some(target_idx)=target_card {
                for path in paths {
                    let trimmed=path.trim();
                    if trimmed.is_empty() {continue;}
                    let new_item=create_shelf_item(trimmed);
                    if self.shelf_items[target_idx].is_group() {
                        if !self.shelf_items[target_idx].children.iter().any(|item| item.path.eq_ignore_ascii_case(trimmed)) {
                            self.shelf_items[target_idx].children.push(new_item);
                            added=true;
                        }
                        let count=self.shelf_items[target_idx].children.len();
                        self.shelf_items[target_idx].size_str=format!("{} items",count);
                    } else {
                        let target_file=self.shelf_items.remove(target_idx);
                        let folder=create_folder_item(vec![target_file,new_item]);
                        self.shelf_items.insert(target_idx,folder);
                        added=true;
                    }
                }
            } else {
                for path in paths {
                    let trimmed=path.trim();
                    if trimmed.is_empty() {continue;}
                    if !self.shelf_items.iter().any(|item| item.path.eq_ignore_ascii_case(trimmed)) {
                        self.shelf_items.push(create_shelf_item(trimmed));
                        added=true;
                    }
                }
            }
        }
        self.ole_drag_target=None;
        if added || !paths.is_empty() {
            self.active_tab=TAB_SHELF;
            if !self.expanded {
                self.expanded=true;
                unsafe { self.layout(hwnd); }
            }
            unsafe { InvalidateRect(hwnd,std::ptr::null(),0); }
        }
    }
    unsafe fn layout(&self,hwnd:Hwnd) {
        let w=self.px(self.panel_w_now()); let h=self.px(self.panel_h_now());
        SetWindowPos(hwnd,-1isize as Hwnd,(GetSystemMetrics(0)-w)/2,0,w,h,0x10);
    }
}
impl Drop for State {
    fn drop(&mut self) {
        if !self.drop_target.is_null() {
            unsafe { let _ = Box::from_raw(self.drop_target); }
            self.drop_target = null_mut();
        }
        self.running.store(false,std::sync::atomic::Ordering::Relaxed);
        self.renderer.take();
        unsafe {for font in self.fonts {DeleteObject(font);}SelectObject(self.dc,self.original);DeleteObject(self.bitmap);DeleteDC(self.dc);}
    }
}
/// Damped spring (zeta < 1 gives Apple's slight settle) integrated in small fixed substeps.
fn spring(pos:&mut f64,vel:&mut f64,target:f64,omega:f64,zeta:f64,dt:f64)->bool {
    if *pos==target && *vel==0. {return false;}
    let dt=dt.min(0.1);let steps=((dt/0.002).ceil() as usize).max(1);let h=dt/steps as f64;
    for _ in 0..steps {*vel+=(-omega*omega*(*pos-target)-2.*zeta*omega**vel)*h;*pos+=*vel*h;}
    if (*pos-target).abs()<0.0005 && vel.abs()<0.005 {*pos=target;*vel=0.;}
    true
}
unsafe fn reduced_motion()->bool {let mut enabled=1i32;SystemParametersInfoW(0x1042,0,(&mut enabled as *mut i32).cast(),0)!=0 && enabled==0}
fn fade(color:u32,opacity:f64)->u32 {let channel=|shift:u32|((((color>>shift)&255u32) as f64*opacity).round() as u32)<<shift;channel(0)|channel(8)|channel(16)}
fn blend_color(c1:u32,c2:u32,t:f64)->u32 {
    let t=t.clamp(0.,1.);
    let r1=(c1&255) as f64; let g1=((c1>>8)&255) as f64; let b1=((c1>>16)&255) as f64;
    let r2=(c2&255) as f64; let g2=((c2>>8)&255) as f64; let b2=((c2>>16)&255) as f64;
    let r=(r1+(r2-r1)*t).round() as u32;
    let g=(g1+(g2-g1)*t).round() as u32;
    let b=(b1+(b2-b1)*t).round() as u32;
    rgb(r,g,b)
}
fn create_shelf_item(path_str:&str)->ShelfItem {
    let p=std::path::Path::new(path_str);
    let name=p.file_name().and_then(|n|n.to_str()).unwrap_or(path_str).to_string();
    let ext=p.extension().and_then(|e|e.to_str()).unwrap_or("").to_lowercase();
    let meta=std::fs::metadata(p);
    let is_dir=meta.as_ref().map(|m|m.is_dir()).unwrap_or(false);
    let size_str=if is_dir {
        "Folder".to_string()
    } else if let Ok(m)=meta {
        let len=m.len();
        if len<1024 { format!("{} B",len) }
        else if len<1024*1024 { format!("{:.1} KB",len as f64/1024.) }
        else if len<1024*1024*1024 { format!("{:.1} MB",len as f64/(1024.*1024.)) }
        else { format!("{:.1} GB",len as f64/(1024.*1024.*1024.)) }
    } else {
        "--".to_string()
    };
    ShelfItem { path:path_str.to_string(), name, ext, size_str, is_dir, children:Vec::new() }
}
/// Starts a standard shell file drag (as Explorer does) for `paths`; blocks until the drop ends.
/// Only copy and link are offered so a drop into Explorer copies instead of moving the file away.
unsafe fn drag_out(hwnd:Hwnd,paths:&[String]) {
    let data=file_data_object(paths);
    if data.is_null() {return;}
    let mut effect=0;
    SHDoDragDrop(hwnd,data,null_mut(),1|4,&mut effect);
    ((*(*(data as *mut IDataObject)).lp_vtbl).release)(data);
}
/// Shell data object (CF_HDROP, Shell IDList, …) for files, as Explorer builds for a drag; null on failure.
unsafe fn file_data_object(paths:&[String])->*mut c_void {
    let mut pidls:Vec<*mut c_void>=Vec::new();
    for path in paths {
        let mut pidl=null_mut();let mut attributes=0;
        if SHParseDisplayName(wide(path).as_ptr(),null_mut(),&mut pidl,0,&mut attributes)>=0 && !pidl.is_null() {pidls.push(pidl);}
    }
    // Absolute ID lists are relative to the Desktop, whose ID list is empty (just the terminator);
    // a null folder yields a data object without CF_HDROP that drop targets reject.
    let desktop:[u16;1]=[0];
    let mut data=null_mut();
    if !pidls.is_empty() && SHCreateDataObject(desktop.as_ptr().cast(),pidls.len() as u32,pidls.as_ptr(),null_mut(),&IID_IDATAOBJECT,&mut data)<0 {data=null_mut();}
    for pidl in pidls {ILFree(pidl);}
    data
}
/// `shelf.txt`: `F<TAB>path` for a file, `G<TAB>path<TAB>path…` for a folder group.
fn serialize_shelf(items:&[ShelfItem])->String {
    items.iter().map(|item| if item.is_group() {
        format!("G\t{}\n",item.children.iter().map(|c|c.path.as_str()).collect::<Vec<_>>().join("\t"))
    } else {
        format!("F\t{}\n",item.path)
    }).collect()
}
/// Groups of paths in shelf order; a single-path group is a standalone file.
fn parse_shelf(src:&str)->Vec<Vec<String>> {
    src.lines().filter_map(|line| {
        let mut parts=line.split('\t');
        let kind=parts.next()?;
        let paths:Vec<String>=parts.filter(|p|!p.is_empty()).map(str::to_string).collect();
        match kind {"F" if paths.len()==1=>Some(paths),"G" if !paths.is_empty()=>Some(paths),_=>None}
    }).collect()
}
/// Restores the saved shelf, dropping files that no longer exist (a group of one becomes a file).
fn load_shelf()->Vec<ShelfItem> {
    let Some(src)=tasks::data_path("shelf.txt").and_then(|p|std::fs::read_to_string(p).ok()) else {return Vec::new()};
    parse_shelf(&src).into_iter().filter_map(|paths| {
        let mut alive:Vec<ShelfItem>=paths.iter().filter(|p|std::path::Path::new(p).exists()).map(|p|create_shelf_item(p)).collect();
        match alive.len() {0=>None,1=>alive.pop(),_=>Some(create_folder_item(alive))}
    }).collect()
}
fn create_folder_item(children:Vec<ShelfItem>)->ShelfItem {
    let count=children.len();
    ShelfItem {
        path:String::new(),
        name:"Folder".to_string(),
        ext:String::new(),
        size_str:format!("{} items",count),
        is_dir:true,
        children,
    }
}
fn classify_file(item:&ShelfItem)->FileKind {
    if item.is_group() || item.is_dir { return FileKind::Folder; }
    match item.ext.as_str() {
        "png"|"jpg"|"jpeg"|"gif"|"bmp"|"webp"|"svg"|"ico"|"tiff"=>FileKind::Image,
        "mp3"|"wav"|"flac"|"aac"|"m4a"|"ogg"|"wma"=>FileKind::Audio,
        "rs"|"js"|"ts"|"jsx"|"tsx"|"py"|"c"|"cpp"|"h"|"hpp"|"cs"|"go"|"html"|"css"|"json"|"toml"|"yaml"|"xml"|"sh"|"bat"|"ps1"|"sql"=>FileKind::Code,
        "zip"|"rar"|"7z"|"tar"|"gz"|"bz2"|"xz"|"iso"=>FileKind::Archive,
        "pdf"|"doc"|"docx"|"txt"|"md"|"rtf"|"xls"|"xlsx"|"ppt"|"pptx"|"csv"=>FileKind::Document,
        _=>FileKind::Generic,
    }
}
fn file_kind_color(kind:FileKind)->u32 {
    match kind {
        FileKind::Folder=>rgb(98,168,240),
        FileKind::Image=>rgb(255,55,95),
        FileKind::Audio=>rgb(255,69,58),
        FileKind::Code=>rgb(191,90,242),
        FileKind::Archive=>rgb(142,142,147),
        FileKind::Document=>BLUE,
        FileKind::Generic=>rgb(142,142,147),
    }
}
fn truncate_filename(name:&str,max_chars:usize)->String {
    let char_count=name.chars().count();
    if char_count<=max_chars { return name.to_string(); }
    if let Some(dot_idx)=name.rfind('.') {
        if dot_idx>0 && dot_idx<name.len()-1 {
            let ext=&name[dot_idx..];
            let base=&name[..dot_idx];
            let ext_len=ext.chars().count();
            if ext_len<max_chars.saturating_sub(4) {
                let keep_base=max_chars.saturating_sub(3+ext_len);
                let truncated_base:String=base.chars().take(keep_base).collect();
                return format!("{}...{}",truncated_base,ext);
            }
        }
    }
    let keep=max_chars.saturating_sub(3);
    let truncated:String=name.chars().take(keep).collect();
    format!("{}...",truncated)
}
unsafe fn open_file(path:&str) {
    ShellExecuteW(null_mut(),wide("open").as_ptr(),wide(path).as_ptr(),null_mut(),null_mut(),1);
}
unsafe fn open_in_explorer(path:&str) {
    let param=wide(&format!("/select,\"{}\"",path));
    ShellExecuteW(null_mut(),wide("open").as_ptr(),wide("explorer.exe").as_ptr(),param.as_ptr(),null_mut(),1);
}
unsafe fn open_file_dialog(hwnd:Hwnd)->Vec<String> {
    let mut buffer=vec![0u16;8192];
    let filter=wide("All Files (*.*)\0*.*\0");
    let title=wide("Add File to Shelf");
    let mut ofn:OpenFileNameW=std::mem::zeroed();
    ofn.struct_size=std::mem::size_of::<OpenFileNameW>() as u32;
    ofn.hwnd_owner=hwnd;
    ofn.filter=filter.as_ptr();
    ofn.file=buffer.as_mut_ptr();
    ofn.max_file=buffer.len() as u32;
    ofn.title=title.as_ptr();
    ofn.flags=0x00080000|0x00001000|0x00000200;
    if GetOpenFileNameW(&mut ofn)!=0 {
        let mut strings=Vec::new();
        let mut start=0;
        for i in 0..buffer.len() {
            if buffer[i]==0 {
                if i>start { strings.push(String::from_utf16_lossy(&buffer[start..i])); }
                start=i+1;
                if i+1<buffer.len() && buffer[i+1]==0 { break; }
            }
        }
        if strings.len()==1 { return strings; }
        else if strings.len()>1 {
            let dir=&strings[0];
            let mut paths=Vec::new();
            for file_name in &strings[1..] {
                let full=std::path::Path::new(dir).join(file_name);
                paths.push(full.to_string_lossy().to_string());
            }
            return paths;
        }
    }
    Vec::new()
}
/// Rendered text, kept as 8-bit coverage masks. GDI text with SF Pro costs ~100 µs a string, which
/// dominated every frame; a cached mask blends in a few µs. Keyed by font, flags, box size and text;
/// entries unused for a few seconds are dropped, so the cache stays small (changing clocks and
/// timers make a new entry every second).
#[derive(Default)]
struct TextCache { masks:std::collections::HashMap<(usize,u32,i32,i32,Vec<u16>),(Vec<u8>,u32)> }
/// Draws `text` like `DrawTextW(dc, text, rect, flags)` with the DC's current font and text color,
/// honoring the DC's clip box, but from a cached grayscale mask.
unsafe fn cached_text(dc:Handle,s:&State,text:&[u16],rect:Rect,flags:u32) {
    let (w,h)=(rect.right-rect.left,rect.bottom-rect.top);
    if text.is_empty() || w<=0 || h<=0 || w>4096 || h>2048 {return;}
    let font=GetCurrentObject(dc,6);let color=GetTextColor(dc);
    let mut cache=s.text_cache.borrow_mut();
    let now=s.ticks;
    if cache.masks.len()>192 {
        cache.masks.retain(|_,(_,used)|now.wrapping_sub(*used)<300);
        if cache.masks.len()>384 {cache.masks.clear();}
    }
    let key=(font as usize,flags,w,h,text.to_vec());
    if !cache.masks.contains_key(&key) {
        let mask=render_mask(font,text,w,h,flags);
        cache.masks.insert(key.clone(),(mask,now));
    }
    let Some((mask,used))=cache.masks.get_mut(&key) else {return};
    *used=now;
    let mut clip=Rect::default();
    if GetClipBox(dc,&mut clip)==0 {return;}
    blend_mask(s,mask,w,rect,clip,color);
}
/// Blends a coverage mask (row length `w`, placed at `rect`) into the frame in `color`, within `clip`.
unsafe fn blend_mask(s:&State,mask:&[u8],w:i32,rect:Rect,clip:Rect,color:u32) {
    let (x0,x1)=(rect.left.max(clip.left).max(0),rect.right.min(clip.right).min(s.stride as i32));
    let (y0,y1)=(rect.top.max(clip.top).max(0),rect.bottom.min(clip.bottom));
    if x0>=x1 || y0>=y1 {return;}
    let (r,g,b)=(color&255,(color>>8)&255,(color>>16)&255);
    for y in y0..y1 {
        let row=std::slice::from_raw_parts_mut(s.pixels.add(y as usize*s.stride),x1 as usize);
        let src=&mask[((y-rect.top)*w) as usize..];
        for x in x0..x1 {
            let c=src[(x-rect.left) as usize] as u32;
            if c==0 {continue;}
            let d=row[x as usize];
            let mix=|dst:u32,col:u32|(dst*(255-c)+col*c+127)/255;
            row[x as usize]=0xff000000|(mix((d>>16)&255,r)<<16)|(mix((d>>8)&255,g)<<8)|mix(d&255,b);
        }
    }
}
/// White-on-black rendering of one text box; its red channel is the coverage GDI would blend with.
unsafe fn render_mask(font:Handle,text:&[u16],w:i32,h:i32,flags:u32)->Vec<u8> {
    let dc=CreateCompatibleDC(null_mut());
    let info=BitmapInfo{size:40,width:w,height:-h,planes:1,bits:32,compression:0,bytes:0,x:0,y:0,used:0,important:0};
    let mut bits=null_mut();
    let bitmap=CreateDIBSection(dc,&info,0,&mut bits,null_mut(),0);
    if bitmap.is_null() || bits.is_null() {DeleteDC(dc);return vec![0;(w*h) as usize];}
    let old_bitmap=SelectObject(dc,bitmap);let old_font=SelectObject(dc,font);
    SetBkMode(dc,1);SetTextColor(dc,0xffffff);
    let mut box_rect=Rect{left:0,top:0,right:w,bottom:h};
    DrawTextW(dc,text.as_ptr(),text.len() as i32,&mut box_rect,flags);
    GdiFlush();
    let pixels=std::slice::from_raw_parts(bits as *const u32,(w*h) as usize);
    let mask=pixels.iter().map(|&p|(((p>>16)&255).max((p>>8)&255).max(p&255)) as u8).collect();
    SelectObject(dc,old_font);SelectObject(dc,old_bitmap);DeleteObject(bitmap);DeleteDC(dc);
    mask
}
unsafe fn text_opt(dc:Handle,s:&State,font:usize,label:&str,color:u32,x:f64,y:f64,w:f64,h:f64,center:bool,opacity:f64,extra_dx:f64) {
    text_align_opt(dc,s,font,label,color,x,y,w,h,if center {1} else {0},opacity,extra_dx);
}
unsafe fn text_align_opt(dc:Handle,s:&State,font:usize,label:&str,color:u32,x:f64,y:f64,w:f64,h:f64,align:u32,opacity:f64,extra_dx:f64) {
    if opacity<=0. {return;}
    let (dx,dy)=s.offset();let (x,y)=(x+dx+extra_dx,y+dy);let color=fade(color,opacity);
    SelectObject(dc,s.fonts[font]); SetTextColor(dc,color);
    let rect=Rect{left:s.px(x),top:s.px(y),right:s.px(x+w),bottom:s.px(y+h)};
    let mut buffer=s.text_buffer.borrow_mut();buffer.clear();buffer.extend(label.encode_utf16());
    GdiFlush();cached_text(dc,s,&buffer,rect,0x8824|align);
}
unsafe fn round_opt(s:&State,x:f64,y:f64,w:f64,h:f64,r:f64,color:u32,opacity:f64,extra_dx:f64) {
    if opacity<=0. || w<=0. || h<=0. {return;}
    let (dx,dy)=s.offset();let (x,y)=(x+dx+extra_dx,y+dy);let color=fade(color,opacity);
    GdiFlush();s.renderer.as_ref().unwrap().round(x,y,w,h,r,color);
}
unsafe fn polygon_opt<const N:usize>(s:&State,points:[(f64,f64);N],color:u32,opacity:f64,extra_dx:f64) {
    if opacity<=0. {return;}
    let (dx,dy)=s.offset();
    GdiFlush();s.renderer.as_ref().unwrap().polygon(points.map(|(x,y)|(x+dx+extra_dx,y+dy)),fade(color,opacity));
}
unsafe fn pill_opt(s:&State,x:f64,y:f64,w:f64,h:f64,color:u32,opacity:f64,extra_dx:f64) {
    round_opt(s,x,y,w,h,w.min(h)/2.,color,opacity,extra_dx);
}
unsafe fn icon_opt(s:&State,index:usize,x:f64,y:f64,color:u32,opacity:f64,extra_dx:f64) {
    if opacity<=0. {return;}
    let (dx,dy)=s.offset();let (x,y)=(x+dx+extra_dx,y+dy);let color=fade(color,opacity);
    cached_icon(s,index,x,y,color);
}
/// Icons as cached coverage masks, like `cached_text`: GDI+'s antialiased strokes cost ~60 µs an icon
/// every frame. Each icon is rendered once per quarter-pixel phase, so moving icons still glide
/// instead of snapping to whole pixels. The speaker's variant (waves or muted) is part of the key.
struct IconCanvas { renderer:render::Renderer, pixels:Box<[u32]> }
const ICON_CANVAS:usize=96; const ICON_PAD:f64=4.;
#[derive(Default)]
struct IconCache { canvas:Option<IconCanvas>, masks:std::collections::HashMap<(usize,u8,u8,u8),Mask> }
/// A coverage mask cropped to its ink, `left`/`top` relative to where the icon was drawn.
struct Mask { left:i32, top:i32, w:i32, h:i32, cov:Vec<u8> }
unsafe fn cached_icon(s:&State,index:usize,x:f64,y:f64,color:u32) {
    let (px,py)=(x*s.scale,y*s.scale);
    let (mut ix,mut iy)=(px.floor(),py.floor());
    let (mut qx,mut qy)=(((px-ix)*4.).round(),((py-iy)*4.).round());
    if qx>=4. {ix+=1.;qx=0.;}
    if qy>=4. {iy+=1.;qy=0.;}
    let variant=if index==8 {if s.vol_muted {4} else {let l=s.vol_level.max(0.);if l<=0.001 {0} else if l<0.34 {1} else if l<0.67 {2} else {3}}} else {0};
    let mut cache=s.icon_cache.borrow_mut();
    let key=(index,variant,qx as u8,qy as u8);
    if !cache.masks.contains_key(&key) {
        let canvas=cache.canvas.get_or_insert_with(|| {
            let mut pixels=vec![0u32;ICON_CANVAS*ICON_CANVAS].into_boxed_slice();
            let renderer=render::Renderer::new(pixels.as_mut_ptr(),ICON_CANVAS,ICON_CANVAS as i32,s.scale);
            IconCanvas{renderer,pixels}
        });
        canvas.pixels.fill(0xff000000);
        let at=|q:f64|(ICON_PAD+q/4.)/s.scale;
        icon_with(s,&canvas.renderer,index,at(qx),at(qy),WHITE);
        let n=ICON_CANVAS as i32;
        let (mut x0,mut y0,mut x1,mut y1)=(n,n,0,0);
        for y in 0..n {for x in 0..n {if canvas.pixels[(y*n+x) as usize]&0xff!=0 {x0=x0.min(x);y0=y0.min(y);x1=x1.max(x+1);y1=y1.max(y+1);}}}
        let mask=if x0>=x1 {Mask{left:0,top:0,w:0,h:0,cov:Vec::new()}} else {
            let cov=(y0..y1).flat_map(|y|(x0..x1).map(move |x|(y,x))).map(|(y,x)|(canvas.pixels[(y*n+x) as usize]&0xff) as u8).collect();
            Mask{left:x0-ICON_PAD as i32,top:y0-ICON_PAD as i32,w:x1-x0,h:y1-y0,cov}
        };
        cache.masks.insert(key,mask);
    }
    let mask=&cache.masks[&key];
    let clip=Rect{left:0,top:0,right:s.stride as i32,bottom:s.px(DIB_H)};
    blend_mask(s,&mask.cov,mask.w,Rect{left:ix as i32+mask.left,top:iy as i32+mask.top,right:ix as i32+mask.left+mask.w,bottom:iy as i32+mask.top+mask.h},clip,color);
}
unsafe fn transport_opt(s:&State,cx:f64,cy:f64,index:usize,playing:bool,color:u32,opacity:f64,extra_dx:f64) {
    if opacity<=0. {return;}
    let (dx,dy)=s.offset();let (cx,cy)=(cx+dx+extra_dx,cy+dy);let color=fade(color,opacity);
    GdiFlush();let r=s.renderer.as_ref().unwrap();
    match index {
        0 => {
            r.polygon([(cx,cy-6.5),(cx,cy+6.5),(cx-10.,cy)],color);
            r.polygon([(cx+10.,cy-6.5),(cx+10.,cy+6.5),(cx,cy)],color);
        }
        1 => {
            if playing {
                r.round(cx-6.,cy-8.,4.,16.,1.2,color);
                r.round(cx+2.,cy-8.,4.,16.,1.2,color);
            } else {
                r.polygon([(cx-5.,cy-8.5),(cx-5.,cy+8.5),(cx+9.,cy)],color);
            }
        }
        2 => {
            r.polygon([(cx-10.,cy-6.5),(cx-10.,cy+6.5),(cx,cy)],color);
            r.polygon([(cx,cy-6.5),(cx,cy+6.5),(cx+10.,cy)],color);
        }
        _ => {}
    }
}
unsafe fn draw_file_icon_opt(s:&State,kind:FileKind,x:f64,y:f64,color:u32,bg_color:u32,opacity:f64,extra_dx:f64) {
    if opacity<=0. {return;}
    let (dx,dy)=s.offset();let (x,y)=(x+dx+extra_dx,y+dy);
    let color=fade(color,opacity);let bg_color=fade(bg_color,opacity);
    GdiFlush();let r=s.renderer.as_ref().unwrap();
    match kind {
        FileKind::Folder => {
            r.round(x,y+2.,6.,3.,1.,color);
            r.round(x,y+4.,16.,11.,2.,color);
            r.round(x+1.5,y+6.,13.,8.,1.,bg_color);
            r.polygon([(x+1.,y+7.5),(x+15.,y+7.5),(x+13.5,y+13.5),(x+2.5,y+13.5)],color);
        }
        FileKind::Image => {
            r.round(x,y+1.,16.,13.,2.,color);
            r.round(x+1.5,y+2.5,13.,10.,1.,bg_color);
            r.polygon([(x+3.,y+11.),(x+7.,y+6.),(x+11.,y+11.)],color);
            r.polygon([(x+9.,y+11.),(x+12.,y+7.5),(x+14.,y+11.)],color);
            r.round(x+10.,y+3.5,3.,3.,1.5,color);
        }
        FileKind::Audio => {
            r.round(x+5.,y+2.,2.,9.,0.8,color);
            r.round(x+12.,y,2.,9.,0.8,color);
            r.polygon([(x+5.,y+2.),(x+14.,y),(x+14.,y+3.),(x+5.,y+5.)],color);
            r.round(x+2.,y+8.,5.,4.,2.,color);
            r.round(x+9.,y+6.,5.,4.,2.,color);
        }
        FileKind::Code => {
            r.polygon([(x+4.5,y+3.),(x+1.5,y+7.5),(x+4.5,y+12.),(x+6.,y+10.5),(x+3.8,y+7.5),(x+6.,y+4.5)],color);
            r.polygon([(x+11.5,y+3.),(x+14.5,y+7.5),(x+11.5,y+12.),(x+10.,y+10.5),(x+12.2,y+7.5),(x+10.,y+4.5)],color);
            r.polygon([(x+8.5,y+2.),(x+10.,y+2.),(x+7.5,y+13.),(x+6.,y+13.)],color);
        }
        FileKind::Archive => {
            r.round(x+1.,y+1.,14.,14.,2.,color);
            r.round(x+6.5,y+2.,3.,12.,0.5,bg_color);
            r.round(x+6.5,y+3.,1.5,1.5,0.5,color);
            r.round(x+8.,y+5.,1.5,1.5,0.5,color);
            r.round(x+6.5,y+7.,1.5,1.5,0.5,color);
            r.round(x+8.,y+9.,1.5,1.5,0.5,color);
            r.round(x+6.,y+11.,4.,3.,1.,color);
        }
        FileKind::Document | FileKind::Generic => {
            r.round(x+2.,y+1.,12.,14.,1.5,color);
            r.polygon([(x+9.,y+1.),(x+14.,y+1.),(x+14.,y+6.)],bg_color);
            r.polygon([(x+9.,y+1.),(x+9.,y+6.),(x+14.,y+6.)],color);
            r.round(x+4.,y+6.,4.,1.5,0.5,bg_color);
            r.round(x+4.,y+9.,7.,1.5,0.5,bg_color);
            r.round(x+4.,y+11.5,5.,1.5,0.5,bg_color);
        }
    }
}
unsafe fn draw_close_button_opt(s:&State,cx:f64,cy:f64,hovered:bool,opacity:f64,extra_dx:f64) {
    if opacity<=0. {return;}
    let (dx,dy)=s.offset();let (cx,cy)=(cx+dx+extra_dx,cy+dy);
    let bg=fade(if hovered {rgb(112,112,118)} else {rgb(66,66,70)},opacity);
    let x_color=fade(rgb(238,238,242),opacity);
    GdiFlush();let r=s.renderer.as_ref().unwrap();
    r.round(cx-8.,cy-8.,16.,16.,8.,bg);
    r.polygon([(cx-3.2,cy-2.2),(cx+2.2,cy+3.2),(cx+3.2,cy+2.2),(cx-2.2,cy-3.2)],x_color);
    r.polygon([(cx+2.2,cy-3.2),(cx-3.2,cy+2.2),(cx-2.2,cy+3.2),(cx+3.2,cy-2.2)],x_color);
}
// Finder-style file tile: a folded page (or blue folder) with the name and size underneath.
unsafe fn shelf_tile(dc:Handle,s:&State,item:&ShelfItem,x:f64,y:f64,opacity:f64,extra_dx:f64) {
    let kind=classify_file(item);
    let cx=x+CARD_W/2.;
    if kind==FileKind::Folder {
        let (fx,fy)=(cx-25.,y+16.);
        round_opt(s,fx,fy,20.,10.,3.,rgb(58,124,204),opacity,extra_dx);
        round_opt(s,fx,fy+4.,50.,38.,5.,rgb(58,124,204),opacity,extra_dx);
        round_opt(s,fx,fy+10.,50.,32.,5.,rgb(98,168,240),opacity,extra_dx);
        if item.is_group() {text_opt(dc,s,3,&item.children.len().to_string(),rgb(232,242,255),fx,fy+12.,50.,30.,true,opacity,extra_dx);}
    } else {
        let (px,py)=(cx-19.,y+10.);let page=rgb(236,236,240);let accent=file_kind_color(kind);
        polygon_opt(s,[(px,py),(px+28.,py),(px+38.,py+10.),(px+38.,py+48.),(px,py+48.)],page,opacity,extra_dx);
        polygon_opt(s,[(px+28.,py),(px+28.,py+10.),(px+38.,py+10.)],rgb(186,186,194),opacity,extra_dx);
        if matches!(kind,FileKind::Document|FileKind::Generic) {
            for (i,w) in [18.,24.,14.].iter().enumerate() {round_opt(s,px+7.,py+14.+i as f64*5.,*w,2.,1.,rgb(196,196,204),opacity,extra_dx);}
        } else {
            draw_file_icon_opt(s,kind,cx-8.,py+10.,accent,page,opacity,extra_dx);
        }
        let ext:String=item.ext.to_uppercase().chars().take(4).collect();
        if !ext.is_empty() {text_opt(dc,s,3,&ext,accent,px,py+31.,38.,14.,true,opacity,extra_dx);}
    }
    text_opt(dc,s,2,&truncate_filename(&item.name,16),WHITE,x+4.,y+66.,CARD_W-8.,18.,true,opacity,extra_dx);
    text_opt(dc,s,3,&item.size_str,SECONDARY,x+4.,y+85.,CARD_W-8.,16.,true,opacity,extra_dx);
}
// Four rounded bars in the artwork's tint, like the Now Playing activity indicator.
unsafe fn waveform(s:&State,x:f64,cy:f64,max_h:f64,color:u32,opacity:f64,energy:f64) {
    if opacity<=0. {return;}
    let t=s.clock.elapsed().as_secs_f64();
    GdiFlush();let r=s.renderer.as_ref().unwrap();
    for i in 0..4 {
        let k=if s.reduced_motion {[0.55,1.,0.7,0.85][i]} else {0.35+0.65*(0.5+0.5*(t*(3.1+i as f64*0.9)+i as f64*1.7).sin())};
        let h=(max_h*k*energy).max(3.);
        r.round(x+i as f64*4.5,cy-h/2.,2.6,h,1.3,fade(color,opacity));
    }
}
// Apple Now Playing: artwork and track text, optional synced lyrics, scrubber, then
// lyrics button / transport / volume along the bottom. Bottom rows follow the panel height.
unsafe fn music_page(dc:Handle,s:&State,dx:f64,o:f64,tint:u32) {
    let lt=s.lyrics_t;let h=s.panel_h;
    let art=ART_SIZE+(LYRIC_ART-ART_SIZE)*lt;
    let tx=MEDIA_X+art+16.-2.*lt;
    let wave_w=if s.media.playing {26.} else {0.};
    text_opt(dc,s,0,&s.media.title,WHITE,tx,46.-1.*lt,MEDIA_R-tx-wave_w,22.,false,o,dx);
    text_opt(dc,s,1,&s.media.artist,SECONDARY,tx,67.-3.*lt,MEDIA_R-tx-wave_w*lt,20.,false,o,dx);
    if !s.media.album.is_empty() {text_opt(dc,s,2,&s.media.album,TERTIARY,tx,87.,MEDIA_R-tx,18.,false,o*(1.-lt).max(0.),dx);}
    if s.media.playing {let (_,oy)=s.offset();waveform(s,MEDIA_R-16.+dx,57.+oy,13.,tint,o,1.);}
    if lt>0.001 {let k=lt.clamp(0.,1.);lyrics_view(dc,s,dx,o*k*k*(3.-2.*k),100.,h-84.);}

    let seek_y=h-66.;let cy=h-26.;
    let dur=s.media.duration as f64;
    let pos=s.play_pos.clamp(0.,dur.max(0.));
    let frac=if dur>0. {pos/dur} else {0.};
    let bar_w=MEDIA_R-MEDIA_X;
    let bar_h=if s.hover==8 {6.} else {4.};
    pill_opt(s,MEDIA_X,seek_y-bar_h/2.,bar_w,bar_h,rgb(52,52,55),o,dx);
    if frac>0. {pill_opt(s,MEDIA_X,seek_y-bar_h/2.,(bar_w*frac).max(bar_h),bar_h,if s.hover==8 {WHITE} else {rgb(230,230,234)},o,dx);}
    let time=|n:i64|format!("{}:{:02}",n.max(0)/60,n.max(0)%60);
    text_opt(dc,s,3,&time(pos as i64),TERTIARY,MEDIA_X,seek_y+5.,60.,16.,false,o,dx);
    let remaining=if s.media.duration>0 {format!("-{}",time(s.media.duration-pos as i64))} else {time(0)};
    text_align_opt(dc,s,3,&remaining,TERTIARY,MEDIA_R-60.,seek_y+5.,60.,16.,2,o,dx);

    for (index,&cx) in TRANSPORT_X.iter().enumerate() {
        let is_hover=s.hover==index as i32;let is_pressed=s.pressed==index as i32;
        if is_hover || is_pressed {pill_opt(s,cx-20.,cy-20.,40.,40.,if is_pressed {rgb(48,48,52)} else {rgb(30,30,32)},o,dx);}
        transport_opt(s,cx,cy,index,s.media.playing,WHITE,o,dx);
    }

    // Lyrics toggle: a white disc with a dark glyph when on, as in Apple Music.
    let hot=s.hover==5 || s.pressed==5;
    let available=matches!(s.lyrics,lyrics::Status::Synced(_));
    let glyph=if s.lyrics_on {
        pill_opt(s,LYRICS_BTN_X-15.,cy-15.,30.,30.,if hot {rgb(214,214,218)} else {WHITE},o,dx);
        rgb(0,0,0)
    } else {
        if hot {pill_opt(s,LYRICS_BTN_X-15.,cy-15.,30.,30.,rgb(30,30,32),o,dx);}
        if hot {WHITE} else if available {SECONDARY} else {TERTIARY}
    };
    icon_opt(s,9,LYRICS_BTN_X-9.,cy-9.,glyph,o,dx);

    if s.vol_level>=0. {
        if s.hover==6 || s.pressed==6 {pill_opt(s,SPEAKER_X-14.,cy-14.,28.,28.,rgb(30,30,32),o,dx);}
        icon_opt(s,8,SPEAKER_X-9.,cy-9.,if s.hover==6 {WHITE} else {SECONDARY},o,dx);
        let active=s.hover==7 || s.vol_drag;
        let th=if active {6.} else {4.};
        let level=if s.vol_muted {0.} else {s.vol_level as f64};
        pill_opt(s,VOL_X0,cy-th/2.,VOL_X1-VOL_X0,th,rgb(52,52,55),o,dx);
        if level>0.005 {pill_opt(s,VOL_X0,cy-th/2.,((VOL_X1-VOL_X0)*level).max(th),th,if active {WHITE} else {rgb(230,230,234)},o,dx);}
    }
}
/// Apple Music-style lyrics: the sung line is bright, others dim, and the column glides on a spring.
unsafe fn lyrics_view(dc:Handle,s:&State,dx:f64,o:f64,top:f64,bottom:f64) {
    if o<=0.001 || bottom-top<4. {return;}
    let color=if s.lyrics_retryable() && s.hover==4 {SECONDARY} else {TERTIARY};
    let message=|text:&str|text_opt(dc,s,1,text,color,MEDIA_X,top,MEDIA_R-MEDIA_X,(bottom-top).max(20.),true,o,dx);
    let lines=match &s.lyrics {
        _ if s.lyrics_key.is_empty()=>return message("Play a song on Spotify to see lyrics"),
        lyrics::Status::Loading=>return message("Loading lyrics…"),
        lyrics::Status::Busy=>return message("LRCLIB is busy · Retrying…"),
        lyrics::Status::Plain=>return message("Lyrics for this song aren't time-synced"),
        lyrics::Status::Instrumental=>return message("Instrumental"),
        lyrics::Status::NotFound=>return message("No lyrics found on LRCLIB · Click to retry"),
        lyrics::Status::Failed=>return message("Couldn't reach LRCLIB · Click to retry"),
        lyrics::Status::Synced(lines)=>lines,
    };
    let n=lines.len();
    let width=MEDIA_R-MEDIA_X;
    {
        let mut heights=s.lyric_heights.borrow_mut();
        if heights.len()!=n {
            heights.clear();
            SelectObject(dc,s.fonts[6]);
            for line in lines {
                if line.text.is_empty() {heights.push(22.);continue;}
                let wide:Vec<u16>=line.text.encode_utf16().collect();
                let mut rect=Rect{left:0,top:0,right:s.px(width),bottom:0};
                DrawTextW(dc,wide.as_ptr(),wide.len() as i32,&mut rect,0x400|0x10|0x800);
                heights.push((rect.bottom as f64/s.scale).max(24.));
            }
        }
    }
    let heights=s.lyric_heights.borrow();
    let mut tops=Vec::with_capacity(n);let mut acc=0.;
    for h in heights.iter() {tops.push(acc);acc+=h+12.;}
    let sc=s.lyric_scroll;
    let base=sc.max(0.);let i0=(base.floor() as usize).min(n-1);let frac=base-i0 as f64;
    let next=if i0+1<n {tops[i0+1]} else {tops[i0]};
    let off=tops[i0]+(next-tops[i0])*frac;
    let anchor=top+34.;
    let (_,oy)=s.offset();
    IntersectClipRect(dc,s.px(dx),s.px(top+oy),s.px(dx+NP_W),s.px(bottom+oy));
    let ss=|x:f64|{let k=x.clamp(0.,1.);k*k*(3.-2.*k)};
    let mut hits=s.lyric_hits.borrow_mut();hits.clear();
    let t=s.clock.elapsed().as_secs_f64();
    for (i,line) in lines.iter().enumerate() {
        let y=anchor+tops[i]-off;let h=heights[i];
        if y+h<top-40. || y>bottom+40. {continue;}
        let emphasis=(1.-(i as f64-sc).abs()).clamp(0.,1.);
        let edge=ss((y+h-top)/40.)*ss((bottom-y)/36.);
        let lo=o*(0.3+0.7*emphasis)*edge;
        if lo<=0.004 {continue;}
        if line.text.is_empty() {
            // Instrumental break: three breathing dots.
            for k in 0..3 {
                let pulse=0.5+0.5*(t*3.-k as f64*0.6).sin();
                pill_opt(s,MEDIA_X+k as f64*13.,y+7.,8.,8.,WHITE,lo*(0.45+0.55*pulse),dx);
            }
        } else {
            SelectObject(dc,s.fonts[6]);SetTextColor(dc,fade(WHITE,lo));
            let wide:Vec<u16>=line.text.encode_utf16().collect();
            let rect=Rect{left:s.px(MEDIA_X+dx),top:s.px(y+oy),right:s.px(MEDIA_R+dx),bottom:s.px(y+h+oy+4.)};
            GdiFlush();cached_text(dc,s,&wide,rect,0x10|0x800);
        }
        if y+h>top && y<bottom {hits.push((y.max(top),(y+h+6.).min(bottom),i));}
    }
    SelectClipRgn(dc,null_mut());
}
// Apple Calendar widget layout: red weekday, large date and month on the left, month grid on the right.
unsafe fn calendar_page(dc:Handle,s:&State,dx:f64,o:f64) {
    let mut now=Time::default();GetLocalTime(&mut now);
    let weekdays=["SUNDAY","MONDAY","TUESDAY","WEDNESDAY","THURSDAY","FRIDAY","SATURDAY"];
    let months=["January","February","March","April","May","June","July","August","September","October","November","December"];
    let month=(now.month as usize).clamp(1,12);
    text_opt(dc,s,3,weekdays[(now.weekday as usize).min(6)],RED,24.,46.,180.,16.,false,o,dx);
    text_opt(dc,s,4,&now.day.to_string(),WHITE,21.,60.,180.,52.,false,o,dx);
    text_opt(dc,s,1,&format!("{} {}",months[month-1],now.year),SECONDARY,24.,112.,180.,20.,false,o,dx);
    let leap=now.year%4==0 && (now.year%100!=0 || now.year%400==0);
    let days=[31,if leap {29}else{28},31,30,31,30,31,31,30,31,30,31][month-1];
    let col=26.;let gx=CAL_W-24.-col*7.;
    for (i,label) in ["M","T","W","T","F","S","S"].iter().enumerate() {text_opt(dc,s,3,label,TERTIARY,gx+i as f64*col,44.,col,16.,true,o,dx);}
    // Monday-based column of the 1st (SYSTEMTIME weekday is Sunday = 0).
    let first=((now.weekday as i32-(now.day as i32-1)%7+7)%7+6)%7;
    for day in 1..=days {
        let cell=first+day-1;let (c,r)=((cell%7) as f64,(cell/7) as f64);
        let (x,y)=(gx+c*col,62.+r*20.);
        let today=day==now.day as i32;
        if today {pill_opt(s,x+col/2.-10.,y,20.,20.,RED,o,dx);}
        text_opt(dc,s,if today {3} else {2},&day.to_string(),if today {WHITE} else if c>=5. {TERTIARY} else {SECONDARY},x,y,col,20.,true,o,dx);
    }
}
unsafe fn shelf_page(dc:Handle,s:&State,dx:f64,o:f64) {
    if let Some(folder_idx)=s.active_folder {
        if folder_idx<s.shelf_items.len() && s.shelf_items[folder_idx].is_group() {
            let bx=s.breadcrumb_x();
            let bc_bg=if s.pressed==105 {Some(rgb(40,40,43))} else if s.breadcrumb_hover {Some(rgb(28,28,30))} else {None};
            if let Some(bg)=bc_bg {pill_opt(s,bx,10.,130.,24.,bg,o,dx);}
            let label=format!("‹  Shelf  /  {}",s.shelf_items[folder_idx].name);
            text_opt(dc,s,2,&label,if s.breadcrumb_hover {WHITE} else {SECONDARY},bx,12.,130.,20.,true,o,dx);
        }
    }
    let items=s.current_items();
    if items.is_empty() {
        let (zx,zy,zw,zh)=(24.,CARD_Y,EXP_W-48.,EXP_H-16.-CARD_Y);
        let hover=s.add_button_hover;
        round_opt(s,zx,zy,zw,zh,16.,if hover {rgb(78,78,84)} else {rgb(46,46,50)},o,dx);
        round_opt(s,zx+1.,zy+1.,zw-2.,zh-2.,15.,if hover && s.pressed==200 {rgb(24,24,26)} else if hover {rgb(16,16,18)} else {rgb(8,8,9)},o,dx);
        icon_opt(s,4,EXP_W/2.-12.,zy+30.,if hover {WHITE} else {SECONDARY},o,dx);
        text_opt(dc,s,0,"Drop files here",WHITE,zx,zy+60.,zw,22.,true,o,dx);
        text_opt(dc,s,2,"or click to choose files",SECONDARY,zx,zy+82.,zw,18.,true,o,dx);
        return;
    }
    let count=items.len();
    let total_w=count as f64*CARD_W+(count.saturating_sub(1)) as f64*CARD_GAP;
    let start_x=if count<=4 {(EXP_W-total_w)/2.} else {24.-s.shelf_scroll};
    for (idx,item) in items.iter().enumerate() {
        let card_x=start_x+idx as f64*(CARD_W+CARD_GAP);
        let card_y=CARD_Y;
        if card_x+CARD_W<0. || card_x>EXP_W {continue;}
        let is_drag_source=s.is_dragging && s.drag_source==Some(idx) && s.active_folder.is_none();
        let co=if is_drag_source {o*0.3} else {o};
        let is_drop_target=s.active_folder.is_none() && (s.drag_target==Some(idx) || s.ole_drag_target==Some(idx));
        let is_hover=s.shelf_hover==idx as i32 && !s.is_dragging;
        let is_rem_hover=s.shelf_remove_hover==idx as i32 && !s.is_dragging;
        if is_drop_target {
            round_opt(s,card_x,card_y,CARD_W,CARD_H,14.,BLUE,co,dx);
            round_opt(s,card_x+2.,card_y+2.,CARD_W-4.,CARD_H-4.,12.,blend_color(0,BLUE,0.22),co,dx);
        } else if is_hover || is_rem_hover {
            round_opt(s,card_x,card_y,CARD_W,CARD_H,14.,rgb(30,30,32),co,dx);
        }
        shelf_tile(dc,s,item,card_x,card_y,co,dx);
        if !s.is_dragging && (is_hover || is_rem_hover) {
            draw_close_button_opt(s,card_x+CLOSE_DX,card_y+CLOSE_DY,is_rem_hover,co,dx);
        }
    }
    if s.is_dragging && s.active_folder.is_none() {
        if let Some(src_idx)=s.drag_source {
            if src_idx<s.shelf_items.len() {
                let (mx,my)=s.drag_current_pos;
                let (fx,fy)=(mx-CARD_W*0.5,my-CARD_H*0.5);
                round_opt(s,fx,fy,CARD_W,CARD_H,14.,rgb(44,44,48),0.96*o,0.);
                shelf_tile(dc,s,&s.shelf_items[src_idx],fx,fy,0.96*o,0.);
            }
        }
    }
}
unsafe fn ring_opt(s:&State,cx:f64,cy:f64,r:f64,w:f64,start:f64,sweep:f64,color:u32,opacity:f64,extra_dx:f64) {
    if opacity<=0. {return;}
    let (dx,dy)=s.offset();
    GdiFlush();s.renderer.as_ref().unwrap().ring(cx+dx+extra_dx,cy+dy,r,w,start,sweep,fade(color,opacity));
}
unsafe fn text_width(dc:Handle,s:&State,font:usize,label:&str)->f64 {
    SelectObject(dc,s.fonts[font]);
    let wide:Vec<u16>=label.encode_utf16().collect();
    let mut size=Size{x:0,y:0};
    GetTextExtentPoint32W(dc,wide.as_ptr(),wide.len() as i32,&mut size);
    size.x as f64/s.scale
}
/// Text in the collapsed notch: no reveal offset, vertically centered in the strip.
unsafe fn collapsed_text(dc:Handle,s:&State,font:usize,label:&str,color:u32,x:f64,w:f64,align:u32,opacity:f64) {
    if opacity<=0. {return;}
    SelectObject(dc,s.fonts[font]);SetTextColor(dc,fade(color,opacity));
    let rect=Rect{left:s.px(x),top:0,right:s.px(x+w),bottom:s.px(COL_H)};
    let mut buffer=s.text_buffer.borrow_mut();buffer.clear();buffer.extend(label.encode_utf16());
    GdiFlush();cached_text(dc,s,&buffer,rect,0x8824|align);
}
/// One lyric line centered in the collapsed notch, offset vertically by `dy`; an empty line shows three dots.
unsafe fn collapsed_line(dc:Handle,s:&State,text:&str,x:f64,w:f64,dy:f64,opacity:f64) {
    if opacity<=0. {return;}
    if text.is_empty() {
        GdiFlush();let r=s.renderer.as_ref().unwrap();
        for k in 0..3 {r.round(x+w/2.-12.+k as f64*9.,COL_H/2.-3.+dy,6.,6.,3.,fade(WHITE,opacity*0.8));}
        return;
    }
    SelectObject(dc,s.fonts[7]);SetTextColor(dc,fade(WHITE,opacity));
    let rect=Rect{left:s.px(x),top:s.px(dy),right:s.px(x+w),bottom:s.px(COL_H+dy)};
    let mut buffer=s.text_buffer.borrow_mut();buffer.clear();buffer.extend(text.encode_utf16());
    GdiFlush();cached_text(dc,s,&buffer,rect,0x8825);
}
fn mmss(secs:f64)->String {let t=secs.ceil().max(0.) as i64;format!("{}:{:02}",t/60,t%60)}
fn phase_color(phase:tasks::Phase)->u32 {if phase==tasks::Phase::Focus {ORANGE} else {GREEN}}
/// Pomodoro length stepper, right of the status line.
const POMO_STEP_X:f64=416.-72.; const POMO_STEP_Y:f64=100.; const POMO_STEP_W:f64=72.; const POMO_STEP_H:f64=26.;
fn pomo_hit(x:f64,y:f64)->i32 {
    let sw=224./3.;
    if (46. ..74.).contains(&y) && (192. ..416.).contains(&x) {return 4000+((x-192.)/sw).floor().min(2.) as i32;}
    if (POMO_STEP_Y..POMO_STEP_Y+POMO_STEP_H).contains(&y) && (POMO_STEP_X..POMO_STEP_X+POMO_STEP_W).contains(&x) {
        return if x<POMO_STEP_X+POMO_STEP_W/2. {4020} else {4021};
    }
    if (136. ..172.).contains(&y) {
        if (192. ..296.).contains(&x) {return 4010;}
        if (312. ..416.).contains(&x) {return 4011;}
    }
    -1
}
// Apple Reminders-style list: a rounded input field, then rows with a circular checkbox.
unsafe fn todo_page(dc:Handle,s:&State,dx:f64,o:f64) {
    let (fx,fy,fw,fh)=(24.,44.,TODO_W-48.,30.);
    if s.todo_editing {
        round_opt(s,fx,fy,fw,fh,10.,BLUE,o,dx);
        round_opt(s,fx+1.5,fy+1.5,fw-3.,fh-3.,8.5,rgb(30,30,32),o,dx);
    } else {
        round_opt(s,fx,fy,fw,fh,10.,if s.hover==1000 {rgb(36,36,38)} else {rgb(28,28,30)},o,dx);
    }
    let plus=if s.todo_editing {BLUE} else {TERTIARY};
    round_opt(s,fx+10.,fy+14.,10.,2.,1.,plus,o,dx);
    round_opt(s,fx+14.,fy+10.,2.,10.,1.,plus,o,dx);
    let (tx,tw)=(fx+28.,fw-40.);
    // Long input keeps its tail in view, like a single-line text field.
    let mut shown=s.todo_input.as_str();
    while !shown.is_empty() && text_width(dc,s,1,shown)>tw {let mut chars=shown.chars();chars.next();shown=chars.as_str();}
    if s.todo_input.is_empty() {
        text_opt(dc,s,1,"Add a task",TERTIARY,tx,fy,tw,fh,false,o,dx);
    } else {
        text_opt(dc,s,1,shown,WHITE,tx,fy,tw,fh,false,o,dx);
    }
    if s.todo_editing && (s.clock.elapsed().as_secs_f64()*2.) as i64%2==0 {
        let caret=if shown.is_empty() {0.} else {text_width(dc,s,1,shown)};
        round_opt(s,tx+caret+1.,fy+7.,1.6,fh-14.,0.8,BLUE,o,dx);
    }
    let n=s.todos.len();
    if n==0 {
        text_opt(dc,s,1,"No tasks yet",TERTIARY,0.,ROW_Y0+20.,TODO_W,22.,true,o,dx);
        text_opt(dc,s,2,"Type above and press Enter",rgb(72,72,76),0.,ROW_Y0+42.,TODO_W,18.,true,o,dx);
        return;
    }
    let hovered=if (2000..4000).contains(&s.hover) {Some(((s.hover-2000)%1000) as usize)} else {None};
    for k in 0..TODO_ROWS {
        let i=s.todo_scroll+k;if i>=n {break;}
        let todo=&s.todos[i];
        let y=ROW_Y0+k as f64*ROW_H;let cy=y+ROW_H/2.;
        let hot=hovered==Some(i);
        if hot {round_opt(s,18.,y,TODO_W-36.,ROW_H,8.,rgb(24,24,26),o,dx);}
        if todo.done {
            pill_opt(s,27.,cy-9.,18.,18.,BLUE,o,dx);
            polygon_opt(s,[(31.4,cy+0.2),(34.4,cy+3.4),(40.8,cy-3.),(39.5,cy-4.3),(34.4,cy+0.8),(32.7,cy-1.1)],WHITE,o,dx);
        } else {
            ring_opt(s,36.,cy,9.,1.6,0.,360.,if hot {SECONDARY} else {rgb(92,92,96)},o,dx);
        }
        text_opt(dc,s,1,&todo.text,if todo.done {TERTIARY} else {WHITE},54.,y,TODO_W-106.,ROW_H,false,o,dx);
        if hot {draw_close_button_opt(s,400.,cy,s.hover==3000+i as i32,o,dx);}
    }
    if n>TODO_ROWS {
        let track=ROW_H*TODO_ROWS as f64;let h=(track*TODO_ROWS as f64/n as f64).max(14.);
        let y=ROW_Y0+(track-h)*(s.todo_scroll as f64/(n-TODO_ROWS) as f64);
        pill_opt(s,TODO_W-14.,y,3.,h,rgb(72,72,76),o,dx);
    }
}
// Apple Clock timer layout: progress ring with the countdown, phase picker, and Reset / Start pills.
/// Settings, in the style of iOS / macOS dark mode: inset grouped cards with switches, a segmented
/// control for the music player, and a footer with the version and Quit.
unsafe fn settings_page(dc:Handle,s:&State,dx:f64,o:f64) {
    const CARD:u32=0x1e1c1c; // rgb(28,28,30), the grouped-card fill
    text_opt(dc,s,3,"NOTCH",SECONDARY,36.,44.,140.,16.,false,o,dx);
    text_opt(dc,s,3,"GENERAL",SECONDARY,238.,44.,140.,16.,false,o,dx);
    round_opt(s,24.,SET_CARD_Y,SET_CARD_W,4.*SET_ROW_H,12.,CARD,o,dx);
    round_opt(s,226.,SET_CARD_Y,SET_CARD_W,3.*SET_ROW_H,12.,CARD,o,dx);
    for (k,&(label,_)) in SETTING_ROWS.iter().enumerate() {
        let (cx,ry)=setting_row(k);
        let (hot,pressed)=(s.hover==5000+k as i32,s.pressed==5000+k as i32);
        if hot || pressed {round_opt(s,cx+4.,ry+3.,SET_CARD_W-8.,SET_ROW_H-6.,8.,if pressed {rgb(50,50,54)} else {rgb(40,40,43)},o,dx);}
        text_opt(dc,s,2,label,WHITE,cx+12.,ry,SET_CARD_W-12.-34.-18.,SET_ROW_H,false,o,dx);
        // iOS switch: the track fills with system green and the knob slides across.
        let t=s.switch_t[k].clamp(0.,1.);
        let (tx,ty)=(cx+SET_CARD_W-12.-34.,ry+(SET_ROW_H-20.)/2.);
        pill_opt(s,tx,ty,34.,20.,blend_color(rgb(57,57,61),GREEN,t),o,dx);
        pill_opt(s,tx+2.+14.*s.switch_t[k],ty+2.,16.,16.,WHITE,o,dx);
        let last=k==3 || k==SETTING_ROWS.len()-1;
        if !last {round_opt(s,cx+12.,ry+SET_ROW_H-1.,SET_CARD_W-12.,1.,0.,rgb(44,44,46),o,dx);}
    }
    text_opt(dc,s,3,"Changes save automatically.",SECONDARY,238.,SET_CARD_Y+3.*SET_ROW_H+8.,SET_CARD_W-12.,16.,false,o,dx);

    // Music player: a segmented control whose thumb glides to the chosen app.
    text_opt(dc,s,3,"MUSIC PLAYER",SECONDARY,36.,SET_SEG_Y-20.,200.,16.,false,o,dx);
    let seg_w=(SET_W-48.)/4.;
    round_opt(s,24.,SET_SEG_Y,SET_W-48.,SET_SEG_H,9.,CARD,o,dx);
    round_opt(s,24.+2.+s.seg_t*seg_w,SET_SEG_Y+2.,seg_w-4.,SET_SEG_H-4.,7.,rgb(72,72,76),o,dx);
    let chosen=s.seg_t.round() as usize;
    for (i,player) in settings::Player::ALL.iter().enumerate() {
        let lit=i==chosen || s.hover==5100+i as i32;
        text_opt(dc,s,2,player.label(),if lit {WHITE} else {SECONDARY},24.+i as f64*seg_w,SET_SEG_Y,seg_w,SET_SEG_H,true,o,dx);
    }

    // Footer: version on the left, Quit on the right.
    text_opt(dc,s,3,concat!("Wisp ",env!("CARGO_PKG_VERSION")),SECONDARY,36.,270.,160.,18.,false,o,dx);
    let (qx,qw)=(SET_W-24.-92.,92.);
    let quit_bg=if s.pressed==5200 {rgb(54,54,58)} else if s.hover==5200 {rgb(44,44,46)} else {CARD};
    pill_opt(s,qx,266.,qw,26.,quit_bg,o,dx);
    text_opt(dc,s,2,"Quit Wisp",WHITE,qx,266.,qw,26.,true,o,dx);
}
unsafe fn pomo_page(dc:Handle,s:&State,dx:f64,o:f64) {
    let now=std::time::Instant::now();
    let p=&s.pomo;let color=phase_color(p.phase);
    let (cx,cy,r,w)=(96.,114.,58.,6.);
    ring_opt(s,cx,cy,r,w,0.,360.,rgb(44,44,46),o,dx);
    let frac=p.fraction(now);
    if frac>0.001 {
        ring_opt(s,cx,cy,r,w,-90.,360.*frac,color,o,dx);
        let rm=r-w/2.;
        for a in [-90f64,-90.+360.*frac] {let t=a.to_radians();pill_opt(s,cx+rm*t.cos()-w/2.,cy+rm*t.sin()-w/2.,w,w,color,o,dx);}
    }
    text_opt(dc,s,5,&mmss(p.remaining(now)),WHITE,cx-56.,cy-24.,112.,36.,true,o,dx);
    text_opt(dc,s,3,p.phase.label(),color,cx-56.,cy+12.,112.,16.,true,o,dx);

    let sw=224./3.;
    round_opt(s,192.,46.,224.,28.,8.,rgb(28,28,30),o,dx);
    for (i,phase) in tasks::Phase::ALL.iter().enumerate() {
        let x=192.+i as f64*sw;let selected=p.phase==*phase;
        if selected {round_opt(s,x+2.,48.,sw-4.,24.,6.,rgb(62,62,66),o,dx);}
        else if s.hover==4000+i as i32 {round_opt(s,x+2.,48.,sw-4.,24.,6.,rgb(38,38,40),o,dx);}
        text_opt(dc,s,2,["Focus","Short","Long"][i],if selected {WHITE} else {SECONDARY},x,46.,sw,28.,true,o,dx);
    }
    let done=p.completed%4;
    text_opt(dc,s,2,"Sessions",SECONDARY,192.,84.,120.,18.,false,o,dx);
    for i in 0..4u32 {pill_opt(s,373.+i as f64*12.,89.5,7.,7.,if i<done {ORANGE} else {rgb(58,58,62)},o,dx);}
    let status=if p.running {
        let mut t=Time::default();GetLocalTime(&mut t);
        let end=t.hour as i64*3600+t.minute as i64*60+t.second as i64+p.remaining(now).ceil() as i64;
        format!("Ends at {}:{:02}",(end/3600)%24,(end/60)%60)
    } else if p.active() {"Paused".to_string()} else {format!("{} minutes",p.minutes(p.phase))};
    text_opt(dc,s,2,&status,TERTIARY,192.,104.,224.,18.,false,o,dx);
    // Between sessions, an iOS-style stepper sets the selected phase's length.
    if !p.active() {
        let (can_less,can_more)=p.can_adjust();
        let (sx,sy,half)=(POMO_STEP_X,POMO_STEP_Y,POMO_STEP_W/2.);
        round_opt(s,sx,sy,POMO_STEP_W,POMO_STEP_H,8.,rgb(28,28,30),o,dx);
        for (i,enabled) in [can_less,can_more].into_iter().enumerate() {
            let hx=sx+i as f64*half;
            let (hot,pressed)=(s.hover==4020+i as i32,s.pressed==4020+i as i32);
            if enabled && (hot || pressed) {round_opt(s,hx+2.,sy+2.,half-4.,POMO_STEP_H-4.,6.,if pressed {rgb(58,58,62)} else {rgb(44,44,46)},o,dx);}
            let color=if enabled {WHITE} else {TERTIARY};
            let (gx,gy)=(hx+half/2.,sy+POMO_STEP_H/2.);
            round_opt(s,gx-5.,gy-0.8,10.,1.6,0.8,color,o,dx);
            if i==1 {round_opt(s,gx-0.8,gy-5.,1.6,10.,0.8,color,o,dx);}
        }
        round_opt(s,sx+half-0.5,sy+5.,1.,POMO_STEP_H-10.,0.,rgb(58,58,62),o,dx);
    }

    let reset_hot=s.hover==4010 || s.pressed==4010;
    pill_opt(s,192.,136.,104.,36.,if reset_hot {rgb(62,62,64)} else {rgb(48,48,50)},o,dx);
    text_opt(dc,s,1,"Reset",if p.active() {WHITE} else {TERTIARY},192.,136.,104.,36.,true,o,dx);
    let start_hot=s.hover==4011 || s.pressed==4011;
    let (bg,fg,label)=if p.running {
        (if start_hot {rgb(86,56,8)} else {rgb(70,44,4)},ORANGE,"Pause")
    } else {
        (if start_hot {rgb(24,74,38)} else {rgb(16,58,28)},GREEN,if p.active() {"Resume"} else {"Start"})
    };
    pill_opt(s,312.,136.,104.,36.,bg,o,dx);
    text_opt(dc,s,1,label,fg,312.,136.,104.,36.,true,o,dx);
}
unsafe fn paint(hwnd:Hwnd,s:&State) {
    let mut ps:Paint=std::mem::zeroed();BeginPaint(hwnd,&mut ps);
    let mut r=Rect::default(); GetClientRect(hwnd,&mut r);
    let dc=s.dc;
    GdiFlush();
    for y in 0..r.bottom as usize {std::slice::from_raw_parts_mut(s.pixels.add(y*s.stride),r.right as usize).fill(0xff000000);}
    SetBkMode(dc,1);
    let tint=s.renderer.as_ref().and_then(|r|r.tint()).unwrap_or(rgb(210,210,215));
    let rv=s.reveal();
    let cur_w=s.panel_w_now();
    let page_left=|tab:usize|(cur_w-page_w(tab))/2.;
    if rv>0. {
        // Header: icon tabs with a capsule that springs between them, overflow menu, Clear actions.
        let mix:[(f64,f64);TAB_COUNT]=std::array::from_fn(|i|s.page_mix(i));
        let (fx,fw)=s.tab_x(s.tab_from);let (tx,tw)=s.tab_x(s.tab_to);let t=s.tab_t;
        if (0..TAB_COUNT as i32).contains(&s.tab_hover) && s.tab_hover as usize!=s.active_tab {let (hx,hw)=s.tab_x(s.tab_hover as usize);pill_opt(s,hx,10.,hw,24.,rgb(28,28,30),rv,0.);}
        pill_opt(s,fx+(tx-fx)*t,10.,fw+(tw-fw)*t,24.,if s.pressed==100+s.active_tab as i32 {rgb(54,54,58)} else {rgb(40,40,43)},rv,0.);
        let lit=|i:usize|blend_color(SECONDARY,WHITE,mix[i].0);
        icon_opt(s,2,34.,13.,lit(TAB_MUSIC),rv,0.);
        icon_opt(s,5,72.,13.,lit(TAB_CAL),rv,0.);
        icon_opt(s,6,110.,13.,lit(TAB_TODO),rv,0.);
        icon_opt(s,7,148.,12.5,lit(TAB_POMO),rv,0.);
        if s.pomo.running {pill_opt(s,165.,25.,5.,5.,phase_color(s.pomo.phase),rv,0.);}
        icon_opt(s,3,186.,12.5,lit(TAB_SHELF),rv,0.);
        icon_opt(s,10,s.tab_x(TAB_SETTINGS).0+8.,13.,lit(TAB_SETTINGS),rv,0.);
        let total_count=s.total_items();
        if total_count>0 {
            let bw=s.shelf_tab_w()-36.;
            pill_opt(s,206.,15.,bw,14.,rgb(66,66,70),rv,0.);
            text_opt(dc,s,3,&total_count.to_string(),WHITE,206.,14.,bw,16.,true,rv,0.);
        }
        for (tab,label) in [(TAB_SHELF,"Clear"),(TAB_TODO,"Clear Done")] {
            let o=mix[tab].0*rv;
            if o<=0. || !s.has_clear(tab) {continue;}
            let (cx,cw)=clear_rect(cur_w,tab);
            let hover=s.tab_hover==10 && s.active_tab==tab;
            if hover {pill_opt(s,cx,10.,cw,24.,rgb(40,40,43),o,0.);}
            text_opt(dc,s,2,label,if hover {WHITE} else {SECONDARY},cx,12.,cw,20.,true,o,0.);
        }
        // Pages sit centered in the panel, so they stay put on screen while its width springs.
        for (tab,&(o,slide)) in mix.iter().enumerate() {
            let o=o*rv;if o<=0.001 {continue;}
            let dx=page_left(tab)+slide;
            match tab {TAB_MUSIC=>music_page(dc,s,dx,o,tint),TAB_CAL=>calendar_page(dc,s,dx,o),TAB_TODO=>todo_page(dc,s,dx,o),TAB_POMO=>pomo_page(dc,s,dx,o),TAB_SETTINGS=>settings_page(dc,s,dx,o),_=>shelf_page(dc,s,dx,o)}
        }
    }
    let p=s.progress;
    let play_t=s.play_progress;
    let (music_o,music_dx)=s.page_mix(TAB_MUSIC);
    let collapsed_art_opacity=play_t*(1.0-rv)*s.collapsed_fade();
    let expanded_art_opacity=music_o*rv;
    let art_opacity=(collapsed_art_opacity+expanded_art_opacity).clamp(0.0,1.0);
    if art_opacity>0.001 {
        let base_x=12.+(page_left(TAB_MUSIC)+MEDIA_X+music_dx-12.)*p;let full_art=ART_SIZE+(LYRIC_ART-ART_SIZE)*s.lyrics_t;let base_y=7.+37.*p;let base_size=22.+(full_art-22.)*p;
        let art_scale=if p<1.0 {0.85+0.15*art_opacity} else {1.0};
        let art_size=base_size*art_scale;
        let art_x=base_x+(base_size-art_size)*0.5;
        let art_y=base_y+(base_size-art_size)*0.5;
        artwork(s,art_x,art_y,art_size,6.+(8.-4.*s.lyrics_t)*p,art_opacity);
    }
    let indicator_p=(1.-p/0.55).clamp(0.,1.);let indicator_p=indicator_p*indicator_p*(3.-2.*indicator_p)*s.collapsed_fade();
    waveform(s,cur_w-30.,COL_H/2.,12.,tint,play_t*indicator_p*(1.-s.timer_t),0.6+0.4*play_t);
    // Collapsed lyrics: the outgoing line rises and fades as the next rises in, driven by lyric_scroll.
    let lyric_o=s.clyric_t*indicator_p;
    if lyric_o>0.001 {
        if let lyrics::Status::Synced(lines)=&s.lyrics {
            let side=if s.timer_t>0.5 {100.} else {50.};
            let w=(cur_w-2.*side).max(20.);
            let sc=s.lyric_scroll.max(0.);let i0=sc.floor() as usize;
            let t={let k=(sc-i0 as f64).clamp(0.,1.);k*k*(3.-2.*k)};
            for (idx,dy,o) in [(i0,-12.*t,1.-t),(i0+1,12.*(1.-t),t)] {
                if o>0.01 {if let Some(line)=lines.get(idx) {collapsed_line(dc,s,&line.text,side,w,dy,lyric_o*o);}}
            }
        }
    }
    // Live timer: countdown on the right; a small progress ring on the left unless artwork is there.
    let timer_o=s.timer_t*indicator_p;
    if timer_o>0.001 {
        let now=std::time::Instant::now();let color=phase_color(s.pomo.phase);
        collapsed_text(dc,s,0,&mmss(s.pomo.remaining(now)),color,cur_w-86.,72.,2,timer_o);
        let ring_o=timer_o*(1.-play_t);
        if ring_o>0.001 {
            GdiFlush();let r=s.renderer.as_ref().unwrap();
            r.ring(24.,COL_H/2.,8.,2.2,0.,360.,fade(blend_color(0,color,0.3),ring_o));
            let f=s.pomo.fraction(now);
            if f>0.001 {r.ring(24.,COL_H/2.,8.,2.2,-90.,360.*f,fade(color,ring_o));}
        }
    }
    // The clock stays centered while music plays; only a collapsed lyric line takes its place.
    let clock_opacity=((1.0-s.clyric_t)*(1.0-p/0.5)).clamp(0.0,1.0)*s.collapsed_fade();
    if clock_opacity>0. {clock(dc,s,clock_opacity);}
    notice_view(dc,s,cur_w);
    GdiFlush();
    let width=r.right as usize;let height=r.bottom as usize;
    // The bottom corners open up as the notch grows, like the hardware notch flowing into a panel.
    let shoulder=6.*s.scale;let radius=((10.+16.*p)*s.scale).min(r.bottom as f64/2.);
    for y in 0..height {
        // Integrate coverage vertically only at the silhouette; no full-surface supersampling.
        let insets: [f64;8]=std::array::from_fn(|sample| {
            let fy=y as f64+(sample as f64+0.5)/8.;
            if fy<shoulder {(shoulder*shoulder-(fy-shoulder).powi(2)).max(0.).sqrt()}
            else if fy>height as f64-radius {shoulder+radius-(radius*radius-(fy-(height as f64-radius)).powi(2)).max(0.).sqrt()}
            else {shoulder}
        });
        let row=std::slice::from_raw_parts_mut(s.pixels.add(y*s.stride),width);
        // Only the silhouette band at each end of the row needs coverage; the interior is already opaque
        // (the frame starts opaque black, GDI+ fills are opaque, and text goes through `cached_text`).
        let band=(insets.iter().cloned().fold(0.,f64::max)+1.).ceil() as usize;
        let band=band.min(width/2);
        for x in (0..band).chain(width-band..width) {
            let pixel=&mut row[x];
            let edge=(x+1).min(width-x) as f64;
            let coverage=insets.iter().map(|inset|(edge-inset).clamp(0.,1.)).sum::<f64>()/8.;
            let a=(coverage*255.).round() as u32;
            let c=*pixel;*pixel=(a<<24)|((((c>>16)&255)*a/255)<<16)|((((c>>8)&255)*a/255)<<8)|((c&255)*a/255);
        }
    }
    let position=Point{x:(GetSystemMetrics(0)-r.right)/2,y:0};let size=Size{x:r.right,y:r.bottom};
    UpdateLayeredWindow(hwnd,null_mut(),&position,&size,dc,&Point::default(),0,&Blend{op:0,flags:0,alpha:255,format:1},2);
    EndPaint(hwnd,&ps);
}
/// The notification pill: app icon (or the sender's picture), title, one line of text, app name.
unsafe fn notice_view(dc:Handle,s:&State,cur_w:f64) {
    let Some(notice)=&s.notice else {return};
    let o=((s.notice_t-0.35)/0.65).clamp(0.,1.)*(1.-s.progress);
    if o<=0.001 {return;}
    let dy=6.*(1.-o);
    let icon_y=(NOTICE_H-NOTICE_ICON)/2.+dy;
    let radius=if !notice.picture {0.} else if notice.circle {NOTICE_ICON/2.} else {10.};
    GdiFlush();
    let r=s.renderer.as_ref().unwrap();
    if !r.icon(16.,icon_y,NOTICE_ICON,radius,o) {
        r.round(16.,icon_y,NOTICE_ICON,NOTICE_ICON,10.,fade(rgb(44,44,48),o));
        let initial=notice.app.chars().next().map(|c|c.to_uppercase().to_string()).unwrap_or_default();
        line_text(dc,s,0,&initial,WHITE,16.,icon_y,NOTICE_ICON,NOTICE_ICON,1,o);
    }
    let tx=16.+NOTICE_ICON+12.;let right=cur_w-18.;
    let app_w=text_width(dc,s,3,&notice.app).min(110.);
    line_text(dc,s,3,&notice.app,TERTIARY,right-app_w,14.+dy,app_w,20.,2,o);
    line_text(dc,s,0,&notice.title,WHITE,tx,14.+dy,(right-app_w-8.-tx).max(10.),20.,0,o);
    if !notice.body.is_empty() {line_text(dc,s,1,&notice.body,SECONDARY,tx,35.+dy,right-tx,20.,0,o);}
}
/// One line of text, vertically centered in its box and cut with "…"; `align` 0 left, 1 center, 2 right.
unsafe fn line_text(dc:Handle,s:&State,font:usize,label:&str,color:u32,x:f64,y:f64,w:f64,h:f64,align:u32,opacity:f64) {
    if opacity<=0. || w<=0. {return;}
    SelectObject(dc,s.fonts[font]);SetTextColor(dc,fade(color,opacity));
    let rect=Rect{left:s.px(x),top:s.px(y),right:s.px(x+w),bottom:s.px(y+h)};
    let mut buffer=s.text_buffer.borrow_mut();buffer.clear();buffer.extend(label.chars().take(400).collect::<String>().encode_utf16());
    GdiFlush();cached_text(dc,s,&buffer,rect,0x8824|align);
}
unsafe fn clock(dc:Handle,s:&State,opacity:f64) {
    if opacity<=0. {return;}
    SelectObject(dc,s.fonts[0]);
    SetTextColor(dc,fade(WHITE,opacity));
    let mut now=Time::default();
    GetLocalTime(&mut now);
    let hour=if s.settings.clock_24h {now.hour} else {(now.hour+11)%12+1};
    let time=format!("{}:{:02}",hour,now.minute);
    let rect=Rect{left:0,top:0,right:s.px(s.panel_w_now()),bottom:s.px(COL_H)};
    let mut buffer=s.text_buffer.borrow_mut();
    buffer.clear();
    buffer.extend(time.encode_utf16());
    GdiFlush();cached_text(dc,s,&buffer,rect,0x8825);
}
unsafe fn artwork(s:&State,x:f64,y:f64,size:f64,radius:f64,opacity:f64) {
    if opacity<=0. {return;}
    GdiFlush();let r=s.renderer.as_ref().unwrap();
    if !r.artwork(x,y,size,radius,opacity) {
        r.round(x,y,size,size,radius,fade(rgb(36,36,40),opacity));
        if size>=30. {icon_at(s,2,x+size/2.-9.,y+size/2.-9.,fade(SECONDARY,opacity));}
    }
}
/// SF Symbols-style line icons on an 18 pt grid (origin at x,y): antialiased 1.6 pt strokes with round caps.
/// 2 music.note, 3 tray.and.arrow.down, 4 the same tray at drop-zone size, 5 calendar, 6 list.bullet, 7 stopwatch.
unsafe fn icon_at(s:&State,index:usize,x:f64,y:f64,color:u32) {
    GdiFlush();icon_with(s,s.renderer.as_ref().unwrap(),index,x,y,color);
}
unsafe fn icon_with(s:&State,r:&render::Renderer,index:usize,x:f64,y:f64,color:u32) {
    let k=if index==4 {1.35} else {1.};
    let p=|px:f64,py:f64|(x+px*k,y+py*k);
    let w=1.6*k;
    match index {
        2=>{
            r.ellipse(x+6.*k,y+14.1*k,3.4*k,2.6*k,color);
            r.stroke(&[p(9.,14.),p(9.,2.)],false,1.7*k,color);
            r.stroke_bezier([p(9.,2.),p(11.9,2.7),p(14.6,4.3),p(14.1,8.)],1.7*k,color);
        }
        3|4=>{
            r.stroke(&[p(1.2,9.6),p(5.6,9.6),p(6.9,12.),p(11.1,12.),p(12.4,9.6),p(16.8,9.6),p(16.8,15.6),p(1.2,15.6)],true,w,color);
            r.stroke(&[p(9.,1.),p(9.,7.6)],false,w,color);
            r.stroke(&[p(6.1,4.9),p(9.,7.8),p(11.9,4.9)],false,w,color);
        }
        5=>{
            r.stroke_round(x+1.2,y+2.8,15.6,13.8,3.4,w,color);
            r.stroke(&[p(1.2,6.9),p(16.8,6.9)],false,w,color);
            for tx in [5.3,12.7] {r.stroke(&[p(tx,1.),p(tx,4.2)],false,w,color);}
            for dx in [5.4,9.,12.6] {for dy in [10.2,13.3] {r.ellipse(x+dx,y+dy,0.95,0.95,color);}}
        }
        6=>{
            for ry in [4.,9.,14.] {r.ellipse(x+2.6,y+ry,1.3,1.3,color);r.stroke(&[p(6.4,ry),p(16.4,ry)],false,w,color);}
        }
        8=>{
            r.polygon([p(1.4,6.6),p(4.6,6.6),p(9.,2.8),p(9.,15.2),p(4.6,11.4),p(1.4,11.4)],color);
            if s.vol_muted {
                r.stroke(&[p(12.2,6.6),p(16.6,11.4)],false,w,color);r.stroke(&[p(16.6,6.6),p(12.2,11.4)],false,w,color);
            } else {
                let level=s.vol_level.max(0.);
                let waves=if level<=0.001 {0} else if level<0.34 {1} else if level<0.67 {2} else {3};
                if waves>=1 {r.stroke_bezier([p(11.,6.6),p(12.4,7.7),p(12.4,10.3),p(11.,11.4)],w,color);}
                if waves>=2 {r.stroke_bezier([p(13.2,4.6),p(15.5,6.4),p(15.5,11.6),p(13.2,13.4)],w,color);}
                if waves>=3 {r.stroke_bezier([p(15.4,2.6),p(18.4,5.2),p(18.4,12.8),p(15.4,15.4)],w,color);}
            }
        }
        9=>{
            r.stroke_round(x+1.,y+2.,16.,11.4,4.6,w,color);
            r.stroke(&[p(5.,13.4),p(4.2,16.4),p(8.6,13.4)],false,w,color);
            for qx in [6.4,10.8] {r.ellipse(x+qx,y+7.2,1.5,1.5,color);r.stroke(&[p(qx+1.2,7.4),p(qx+0.2,9.8)],false,1.3,color);}
        }
        10=>{
            // gearshape: a ring with eight short teeth and a hub.
            r.stroke_ellipse(x+9.,y+9.,5.2,5.2,w,color);
            r.stroke_ellipse(x+9.,y+9.,2.1,2.1,w,color);
            for k in 0..8 {
                let a=k as f64*std::f64::consts::FRAC_PI_4;
                r.stroke(&[p(9.+5.2*a.cos(),9.+5.2*a.sin()),p(9.+7.7*a.cos(),9.+7.7*a.sin())],false,2.3,color);
            }
        }
        7=>{
            r.stroke_ellipse(x+9.,y+10.4,6.6,6.6,w,color);
            r.stroke(&[p(7.,1.2),p(11.,1.2)],false,w,color);
            r.stroke(&[p(9.,1.2),p(9.,3.6)],false,w,color);
            r.stroke(&[p(14.4,4.8),p(15.6,3.6)],false,w,color);
            r.stroke(&[p(9.,10.4),p(9.,6.8)],false,w,color);
        }
        _=>{}
    }
}
unsafe extern "system" fn procedure(hwnd:Hwnd,msg:u32,w:usize,l:isize)->isize {
    let ptr=GetWindowLongPtrW(hwnd,-21) as *mut State;
    if !ptr.is_null() {
        let s=&mut *ptr;
        // Input (keyboard, mouse, mouse leave, OLE drag) wakes the frame thread from its idle pace at once.
        if (0x100..=0x20e).contains(&msg) || msg==0x2a3 || (0x8004..=0x8007).contains(&msg) {s.pace.store(PACE_MOTION,std::sync::atomic::Ordering::Relaxed);}
        match msg {
            0x14=>return 1,
            0x0f=>{paint(hwnd,s);return 0;}
            0x8001=>{
                s.ticks+=1;
                while let Ok(mut media)=s.receiver.try_recv() {
                    if !s.smoke {
                        let new_track=media.title!=s.media.title || media.artist!=s.media.artist;
                        if media.title==s.media.title && media.artist==s.media.artist && media.album==s.media.album && media.artwork.is_none() {
                            media.artwork=s.media.artwork.take();
                        } else {
                            s.renderer.as_mut().unwrap().set_artwork(media.artwork.as_deref());
                        }
                        s.media=media;
                        // Ease toward each fresh sample instead of jumping, unless it is a new track or a seek.
                        // Some players repeat a stale position while playing; those repeats are ignored so
                        // the local clock keeps running.
                        let sample=s.media.pos_secs;
                        let fresh=(sample-s.last_sample).abs()>0.01;
                        s.last_sample=sample;
                        if new_track || !s.media.playing {s.play_pos=sample;}
                        else if fresh {
                            if (sample-s.play_pos).abs()>1.5 {s.play_pos=sample;} else {s.play_pos+=(sample-s.play_pos)*0.35;}
                        }
                        s.request_lyrics();
                        InvalidateRect(hwnd,std::ptr::null(),0);
                    }
                }
                let replies:Vec<lyrics::Reply>=s.lyrics_rx.as_ref().map(|rx|rx.try_iter().collect()).unwrap_or_default();
                for reply in replies {
                    if reply.key==s.lyrics_key {
                        s.lyrics=reply.status;s.lyric_heights.borrow_mut().clear();
                        InvalidateRect(hwnd,std::ptr::null(),0);
                    }
                }
                if !s.smoke && s.expanded && s.active_tab==TAB_MUSIC && !s.vol_drag && s.ticks%20==0 {
                    // The player's audio session appears once it starts playing, so look again every ~2 s.
                    if s.ticks%120==0 || s.volume.app()!=s.media.app {let app=s.media.app.clone();s.volume.refresh(&app);}
                    let (level,muted)=s.volume.get().unwrap_or((-1.,false));
                    if level!=s.vol_level || muted!=s.vol_muted {s.vol_level=level;s.vol_muted=muted;InvalidateRect(hwnd,std::ptr::null(),0);}
                }
                if s.collapsed_lyrics_active() {
                    let key=(s.lyric_target() as i64,s.timer_t>0.5);
                    if key!=s.clyric_key {
                        s.clyric_key=key;
                        let text=match &s.lyrics {lyrics::Status::Synced(lines)=>lines.get(key.0 as usize).map(|l|l.text.clone()).unwrap_or_default(),_=>String::new()};
                        let text_w=if text.is_empty() {30.} else {text_width(s.dc,s,7,&text)};
                        // Equal margins on both sides keep the centered line clear of artwork and waveform/timer.
                        let side=if key.1 {100.} else {50.};
                        s.lyric_w_target=(2.*side+text_w).clamp(COL_W,MAX_LYRIC_W);
                    }
                }
                if !s.smoke && s.ticks%30==0 {
                    let data=serialize_shelf(&s.shelf_items);
                    if data!=s.shelf_saved {tasks::write_data("shelf.txt",&data);s.shelf_saved=data;}
                }
                let now=std::time::Instant::now();let dt=now.duration_since(s.last_frame).as_secs_f64();s.last_frame=now;
                if s.pomo.tick(now) {
                    // Like a Live Activity alert: chime, open on the timer page, close again after a few seconds.
                    MessageBeep(0x40);
                    if !s.smoke {
                        s.active_tab=TAB_POMO;
                        if !s.expanded {s.expanded=true;s.layout(hwnd);}
                        s.alert_until=Some(now+std::time::Duration::from_secs(5));
                    }
                    InvalidateRect(hwnd,std::ptr::null(),0);
                }
                if let Some(until)=s.alert_until {
                    if now>=until {
                        s.alert_until=None;
                        let mut r=Rect::default();GetWindowRect(hwnd,&mut r);let mut c=Point::default();GetCursorPos(&mut c);
                        if !(c.x>=r.left && c.x<r.right && c.y>=r.top && c.y<r.bottom) && !s.todo_editing && !s.menu_active {
                            s.expanded=false;s.layout(hwnd);InvalidateRect(hwnd,std::ptr::null(),0);
                        }
                    }
                }
                let pomo_sec=s.pomo.remaining(now).ceil() as i64;
                if pomo_sec!=s.last_pomo_sec {s.last_pomo_sec=pomo_sec;InvalidateRect(hwnd,std::ptr::null(),0);}
                if s.todo_editing && s.ticks%15==0 {InvalidateRect(hwnd,std::ptr::null(),0);}
                s.update_notices(hwnd,now);
                s.update_cover(hwnd,now);
                let prev_p=s.progress;let prev_w=s.panel_w;let prev_play=s.play_progress;let prev_timer=s.timer_t;let prev_h=s.panel_h;let prev_lw=s.lyric_w;let prev_ct=s.clyric_t;let prev_nt=s.notice_t;let prev_mini=s.mini_t;
                let prev_tab=s.tab_t;let prev_lt=s.lyrics_t;
                let animating=s.advance(dt);
                let resized=s.progress!=prev_p || s.panel_w!=prev_w || s.play_progress!=prev_play || s.timer_t!=prev_timer || s.panel_h!=prev_h || s.lyric_w!=prev_lw || s.clyric_t!=prev_ct || s.notice_t!=prev_nt || s.mini_t!=prev_mini;
                if animating {
                    if resized {s.layout(hwnd);}
                    InvalidateRect(hwnd,std::ptr::null(),0);
                }
                // How fast the frame thread runs next (see the thread in main): vsync-locked while the notch
                // itself moves, a cheaper timer at the display rate while only the waveform or a lyric line
                // moves (or the panel is open), and ~10 ticks a second when nothing does.
                let motion=s.smoke || resized || s.tab_t!=prev_tab || s.lyrics_t!=prev_lt || s.is_dragging || s.dragging_out;
                let ambient=animating || s.expanded || s.media.playing || s.notice.is_some() || !s.notices.is_empty() || s.alert_until.is_some();
                s.pace.store(if motion {PACE_MOTION} else if ambient {PACE_AMBIENT} else {PACE_IDLE},std::sync::atomic::Ordering::Relaxed);
                if !s.usable() {
                    s.hover=-1;s.pressed=-1;s.shelf_hover=-1;s.shelf_remove_hover=-1;s.tab_hover=-1;s.add_button_hover=false;
                    s.breadcrumb_hover=false;s.ole_drag_target=None;
                }
                // A card drag survives settling animations; only closing the notch or leaving the shelf ends it.
                if s.drag_source.is_some() && (!s.expanded || s.active_tab!=TAB_SHELF) {
                    s.drag_source=None;s.drag_target=None;s.is_dragging=false;ReleaseCapture();
                }
                let mut now_time=Time::default();GetLocalTime(&mut now_time);
                let minute_changed=now_time.minute!=s.last_minute;
                if minute_changed {s.last_minute=now_time.minute;}
                if (!s.expanded && (s.media.playing || s.play_progress>0.) && !s.reduced_motion) || (s.expanded && s.active_tab==TAB_MUSIC && (s.media.playing || s.lyrics_on) && !s.reduced_motion) || (!s.expanded && !s.media.playing && minute_changed) || s.ticks%30==0 {
                    InvalidateRect(hwnd,std::ptr::null(),0);
                }
                if s.smoke {
                    if s.ticks==2 {s.media.playing=true;}
                    if !s.verified && s.play_progress==1. && !s.expanded {s.expanded=true;}
                    if !s.verified && s.progress==1. {
                        let mut r=Rect::default();GetWindowRect(hwnd,&mut r);
                        let geom_ok=r.top==0 && r.right-r.left==s.px(NP_W) && r.bottom-r.top==s.px(EXP_H);
                        if geom_ok && !s.smoke_shelf_verified {
                            SendMessageW(hwnd, 0x8004, 0, 0);
                            let drag_enter_ok = s.expanded && s.active_tab == TAB_SHELF;
                            SendMessageW(hwnd, 0x8005, 0, 0);
                            let drag_leave_ok = !s.expanded;
                            let test_files = vec!["test_shelf_file.txt".to_string()];
                            SendMessageW(hwnd, 0x8006, &test_files as *const _ as usize, 0);
                            let drag_drop_ok = s.expanded && s.active_tab == TAB_SHELF && s.shelf_items.len() == 1;
                            let test_files2 = vec!["test_shelf_file_2.txt".to_string()];
                            SendMessageW(hwnd, 0x8006, &test_files2 as *const _ as usize, 0);
                            let add2_ok = s.shelf_items.len() == 2;
                            s.merge_items(1, 0);
                            let folder_ok = s.shelf_items.len() == 1 && s.shelf_items[0].is_group() && s.shelf_items[0].children.len() == 2;
                            s.active_folder = Some(0);
                            let folder_view_ok = s.current_items().len() == 2;
                            s.remove_folder_child(0, 0);
                            let ungroup_ok = s.active_folder.is_none() && s.shelf_items.len() == 1 && !s.shelf_items[0].is_group();
                            if drag_enter_ok && drag_leave_ok && drag_drop_ok && add2_ok && folder_ok && folder_view_ok && ungroup_ok {
                                s.active_tab = TAB_MUSIC;
                                s.shelf_items.clear();
                                s.smoke_shelf_verified = true;
                                s.smoke_drag_verified = true;
                                s.verified = true;
                                s.expanded = false;
                                s.media.playing = false;
                            }
                        }
                    }
                    if (s.verified && s.progress == 0. && s.play_progress == 0.) || s.clock.elapsed().as_secs_f64() > 4. {
                        let mut r = Rect::default(); GetWindowRect(hwnd, &mut r);
                        let ok = s.verified && s.smoke_shelf_verified && s.smoke_drag_verified && r.top == 0 && r.right - r.left == s.px(CLOCK_W) && r.bottom - r.top == s.px(COL_H) && s.play_progress == 0.;
                        let _ = std::fs::write("smoke-result.txt", format!("{}: layered alpha rendering; expanded {}x{}, collapsed {}x{}; shelf integration & folder grouping verified; mode transition verified; {} compositor ticks in {:.3}s.\n", if ok { "PASS" } else { "FAIL" }, NP_W, EXP_H, CLOCK_W, COL_H, s.ticks, s.clock.elapsed().as_secs_f64()));
                        DestroyWindow(hwnd);
                        return 0;
                    }
                }
                UpdateWindow(hwnd);
                return 0;
            }
            0x200=>{
                let mut tracking=MouseTracking{size:size_of::<MouseTracking>() as u32,flags:2,hwnd,hover_time:0};TrackMouseEvent(&mut tracking);
                if s.notice_shown() {s.notice_hover=true;return 0;}
                if !s.smoke && !s.expanded && s.settings.open_on_hover {
                    s.expanded=true;
                    s.layout(hwnd);
                    InvalidateRect(hwnd,std::ptr::null(),0);
                }
                let x=(l as i16) as f64/s.scale;let y=((l>>16) as i16) as f64/s.scale;
                let mut changed=false;
                if s.vol_drag {s.set_volume_x(x);changed=true;}
                if s.usable() {
                    let hit=if y<HEADER_H {s.header_hit(x,y)} else {-1};
                    let bc_hover=hit==12;
                    if s.breadcrumb_hover!=bc_hover { s.breadcrumb_hover=bc_hover; changed=true; }
                    let tab_hover=if hit==12 {-1} else {hit};
                    if s.tab_hover!=tab_hover {s.tab_hover=tab_hover;changed=true;}
                    if s.active_tab==TAB_MUSIC {
                        let hover=s.music_hit(x,y);
                        if s.hover!=hover {s.hover=hover;changed=true;}
                        if s.shelf_hover!=-1 || s.shelf_remove_hover!=-1 || s.add_button_hover {s.shelf_hover=-1;s.shelf_remove_hover=-1;s.add_button_hover=false;changed=true;}
                    } else if s.active_tab==TAB_TODO || s.active_tab==TAB_POMO || s.active_tab==TAB_SETTINGS {
                        let hover=match s.active_tab {TAB_TODO=>s.todo_hit(x,y),TAB_POMO=>pomo_hit(x,y),_=>s.settings_hit(x,y)};
                        if s.hover!=hover {s.hover=hover;changed=true;}
                    } else if s.active_tab==TAB_SHELF {
                        if s.hover!=-1 {s.hover=-1;changed=true;}
                        if let Some(src_idx)=s.drag_source {
                            let dx=x-s.drag_start_pos.0;
                            let dy=y-s.drag_start_pos.1;
                            if !s.is_dragging && (dx*dx+dy*dy>=16.) {
                                s.is_dragging=true;
                            }
                            let outside=x<0. || y<0. || x>s.panel_w_now() || y>s.panel_h_now();
                            if s.is_dragging && (outside || s.active_folder.is_some()) {
                                let paths=s.drag_paths(src_idx);
                                s.drag_source=None;s.is_dragging=false;s.drag_target=None;
                                ReleaseCapture();
                                InvalidateRect(hwnd,std::ptr::null(),0);
                                s.dragging_out=true;
                                drag_out(hwnd,&paths);
                                s.dragging_out=false;
                                let mut r=Rect::default();GetWindowRect(hwnd,&mut r);let mut c=Point::default();GetCursorPos(&mut c);
                                if !(c.x>=r.left && c.x<r.right && c.y>=r.top && c.y<r.bottom) {
                                    s.expanded=false;s.hover=-1;s.pressed=-1;s.shelf_hover=-1;s.shelf_remove_hover=-1;
                                    s.layout(hwnd);
                                }
                                InvalidateRect(hwnd,std::ptr::null(),0);
                                return 0;
                            }
                            if s.is_dragging {
                                s.drag_current_pos=(x,y);
                                let count=s.shelf_items.len();
                                let card_w=CARD_W; let card_h=CARD_H; let card_spacing=CARD_GAP;
                                let total_w=count as f64*card_w+(count.saturating_sub(1)) as f64*card_spacing;
                                let start_x=if count<=4 {(EXP_W-total_w)/2.} else {24.-s.shelf_scroll};
                                let mut target=None;
                                for idx in 0..count {
                                    if idx==src_idx {continue;}
                                    let card_x=start_x+idx as f64*(card_w+card_spacing);let card_y=CARD_Y;
                                    if x>=card_x && x<=card_x+card_w && y>=card_y && y<=card_y+card_h {
                                        target=Some(idx);
                                        break;
                                    }
                                }
                                if s.drag_target!=target {
                                    s.drag_target=target;
                                }
                                changed=true;
                            }
                        }

                        if !s.is_dragging {
                            let items=s.current_items();
                            let count=items.len();
                            if count==0 {
                                let add_btn=in_drop_zone(x,y);
                                if s.add_button_hover!=add_btn {s.add_button_hover=add_btn;changed=true;}
                                if s.shelf_hover!=-1 || s.shelf_remove_hover!=-1 {s.shelf_hover=-1;s.shelf_remove_hover=-1;changed=true;}
                            } else {
                                let card_w=CARD_W;let card_h=CARD_H;let card_spacing=CARD_GAP;
                                let total_w=count as f64*card_w+(count.saturating_sub(1)) as f64*card_spacing;
                                let start_x=if count<=4 {(EXP_W-total_w)/2.} else {24.-s.shelf_scroll};
                                let mut card_hover=-1; let mut rem_hover=-1;
                                for idx in 0..count {
                                    let card_x=start_x+idx as f64*(card_w+card_spacing);let card_y=CARD_Y;
                                    let rem_dx=x-(card_x+CLOSE_DX);let rem_dy=y-(card_y+CLOSE_DY);
                                    if rem_dx*rem_dx+rem_dy*rem_dy<=100. {card_hover=idx as i32;rem_hover=idx as i32;break;}
                                    if x>=card_x && x<=card_x+card_w && y>=card_y && y<=card_y+card_h {card_hover=idx as i32;break;}
                                }
                                if s.shelf_hover!=card_hover {s.shelf_hover=card_hover;changed=true;}
                                if s.shelf_remove_hover!=rem_hover {s.shelf_remove_hover=rem_hover;changed=true;}
                            }
                        }
                    }
                } else {
                    if s.hover!=-1 || s.shelf_hover!=-1 || s.shelf_remove_hover!=-1 || s.tab_hover!=-1 || s.add_button_hover || s.breadcrumb_hover {
                        s.hover=-1;s.shelf_hover=-1;s.shelf_remove_hover=-1;s.tab_hover=-1;s.add_button_hover=false;s.breadcrumb_hover=false;changed=true;
                    }
                }
                if changed {InvalidateRect(hwnd,std::ptr::null(),0);}
                return 0;
            }
            0x201=>{
                if s.notice_shown() {return 0;}
                let x=(l as i16) as f64/s.scale;let y=((l>>16) as i16) as f64/s.scale;
                if s.usable() {
                    let pressed=if y<HEADER_H {
                        match s.header_hit(x,y) {12=>105,10=>110,t if t>=0=>100+t,_=>-1}
                    } else if s.active_tab==TAB_MUSIC {
                        s.music_hit(x,y)
                    } else if s.active_tab==TAB_TODO {
                        s.todo_hit(x,y)
                    } else if s.active_tab==TAB_POMO {
                        pomo_hit(x,y)
                    } else if s.active_tab==TAB_SETTINGS {
                        s.settings_hit(x,y)
                    } else if s.active_tab!=TAB_SHELF {
                        -1
                    } else if s.shelf_items.is_empty() {
                        if in_drop_zone(x,y) {200} else {-1}
                    } else {
                        let items=s.current_items();
                        let count=items.len();
                        let card_w=CARD_W;let card_h=CARD_H;let card_spacing=CARD_GAP;
                        let total_w=count as f64*card_w+(count.saturating_sub(1)) as f64*card_spacing;
                        let start_x=if count<=4 {(EXP_W-total_w)/2.} else {24.-s.shelf_scroll};
                        let mut clicked_card=None;
                        let mut clicked_rem=false;
                        for idx in 0..count {
                            let card_x=start_x+idx as f64*(card_w+card_spacing);let card_y=CARD_Y;
                            let rem_dx=x-(card_x+CLOSE_DX);let rem_dy=y-(card_y+CLOSE_DY);
                            if rem_dx*rem_dx+rem_dy*rem_dy<=100. {clicked_card=Some(idx);clicked_rem=true;break;}
                            if x>=card_x && x<=card_x+card_w && y>=card_y && y<=card_y+card_h {clicked_card=Some(idx);break;}
                        }
                        if let Some(card_idx)=clicked_card {
                            if !clicked_rem {
                                s.drag_source=Some(card_idx);
                                s.drag_start_pos=(x,y);
                                s.drag_current_pos=(x,y);
                                s.is_dragging=false;
                                s.drag_target=None;
                                SetCapture(hwnd);
                            }
                        }
                        -1
                    };
                    if pressed==7 && s.active_tab==TAB_MUSIC {s.vol_drag=true;SetCapture(hwnd);s.set_volume_x(x);}
                    if s.pressed!=pressed {s.pressed=pressed;InvalidateRect(hwnd,std::ptr::null(),0);}
                }
                return 0;
            }
            0x2a3=>{
                if s.notice_hover {
                    // Leaving a notification gives it a moment before it goes.
                    s.notice_hover=false;
                    if let Some(until)=s.notice_until {s.notice_until=Some(until.max(std::time::Instant::now()+std::time::Duration::from_millis(1200)));}
                    return 0;
                }
                if !s.smoke && !s.menu_active && !s.todo_editing && !s.vol_drag && !s.is_dragging && s.drag_source.is_none() {
                    s.expanded=false;
                    s.hover=-1;s.pressed=-1;s.shelf_hover=-1;s.shelf_remove_hover=-1;s.tab_hover=-1;s.add_button_hover=false;
                    s.breadcrumb_hover=false;s.drag_source=None;s.drag_target=None;s.is_dragging=false;s.ole_drag_target=None;
                    s.layout(hwnd);
                    InvalidateRect(hwnd,std::ptr::null(),0);
                }
                return 0;
            }
            0x202=>{
                if s.notice_shown() {s.open_notice(hwnd);return 0;}
                let x=(l as i16) as f64/s.scale; let y=((l>>16) as i16) as f64/s.scale;
                if s.pressed>=0 {s.pressed=-1;InvalidateRect(hwnd,std::ptr::null(),0);}
                if s.vol_drag {
                    s.vol_drag=false;ReleaseCapture();
                    let mut r=Rect::default();GetWindowRect(hwnd,&mut r);let mut c=Point::default();GetCursorPos(&mut c);
                    if !s.smoke && !(c.x>=r.left && c.x<r.right && c.y>=r.top && c.y<r.bottom) {s.expanded=false;s.hover=-1;s.layout(hwnd);}
                    InvalidateRect(hwnd,std::ptr::null(),0);
                    return 0;
                }
                let had_capture=s.drag_source.is_some();
                if had_capture {
                    ReleaseCapture();
                }
                if s.is_dragging {
                    let src=s.drag_source.take();
                    let tgt=s.drag_target.take();
                    s.is_dragging=false;
                    if let (Some(source_idx),Some(target_idx))=(src,tgt) {
                        s.merge_items(source_idx,target_idx);
                    }
                    InvalidateRect(hwnd,std::ptr::null(),0);
                    let mut r=Rect::default();GetWindowRect(hwnd,&mut r);let mut p=Point::default();GetCursorPos(&mut p);
                    if !(p.x>=r.left && p.x<r.right && p.y>=r.top && p.y<r.bottom) {
                        s.expanded=false;s.hover=-1;s.pressed=-1;s.shelf_hover=-1;s.shelf_remove_hover=-1;
                        s.tab_hover=-1;s.add_button_hover=false;s.breadcrumb_hover=false;
                        s.layout(hwnd);InvalidateRect(hwnd,std::ptr::null(),0);
                    }
                    return 0;
                }
                s.drag_source=None;
                s.drag_target=None;
                s.is_dragging=false;
                if had_capture {
                    let mut r=Rect::default();GetWindowRect(hwnd,&mut r);let mut p=Point::default();GetCursorPos(&mut p);
                    if !(p.x>=r.left && p.x<r.right && p.y>=r.top && p.y<r.bottom) {
                        s.expanded=false;s.hover=-1;s.pressed=-1;s.shelf_hover=-1;s.shelf_remove_hover=-1;
                        s.tab_hover=-1;s.add_button_hover=false;s.breadcrumb_hover=false;
                        s.layout(hwnd);InvalidateRect(hwnd,std::ptr::null(),0);
                        return 0;
                    }
                }
                if y<HEADER_H {
                    if s.usable() {
                        match s.header_hit(x,y) {
                            12=>{
                                s.active_folder=None;s.shelf_scroll=0.;s.shelf_hover=-1;s.shelf_remove_hover=-1;
                                InvalidateRect(hwnd,std::ptr::null(),0);return 0;
                            }
                            t if (0..TAB_COUNT as i32).contains(&t)=>{s.active_tab=t as usize;InvalidateRect(hwnd,std::ptr::null(),0);return 0;}
                            10=>{
                                if s.active_tab==TAB_TODO {
                                    s.todos.retain(|t|!t.done);s.clamp_todo_scroll();s.save_todos();
                                    InvalidateRect(hwnd,std::ptr::null(),0);return 0;
                                }
                                s.shelf_items.clear();s.active_folder=None;s.shelf_scroll=0.;s.shelf_hover=-1;s.shelf_remove_hover=-1;InvalidateRect(hwnd,std::ptr::null(),0);return 0;
                            }
                            _=>{}
                        }
                    }
                    if !s.expanded {
                        s.expanded=true;
                        s.layout(hwnd);
                        InvalidateRect(hwnd,std::ptr::null(),0);
                    }
                } else if s.usable() {
                    if s.active_tab==TAB_MUSIC {
                        let hit=s.music_hit(x,y);
                        s.music_click(x,hit);
                        InvalidateRect(hwnd,std::ptr::null(),0);
                    } else if s.active_tab==TAB_TODO {
                        let hit=s.todo_hit(x,y);
                        s.todo_click(hwnd,hit);
                        InvalidateRect(hwnd,std::ptr::null(),0);
                    } else if s.active_tab==TAB_POMO {
                        s.pomo_click(pomo_hit(x,y));
                        InvalidateRect(hwnd,std::ptr::null(),0);
                    } else if s.active_tab==TAB_SETTINGS {
                        let hit=s.settings_hit(x,y);
                        s.settings_click(hwnd,hit);
                        InvalidateRect(hwnd,std::ptr::null(),0);
                    } else if s.active_tab!=TAB_SHELF {
                    } else if s.shelf_items.is_empty() {
                        if in_drop_zone(x,y) {
                            let paths=open_file_dialog(hwnd);
                            s.add_shelf_items(&paths,hwnd);
                        }
                    } else if let Some(folder_idx)=s.active_folder {
                        if folder_idx<s.shelf_items.len() && s.shelf_items[folder_idx].is_group() {
                            let count=s.shelf_items[folder_idx].children.len();
                            let card_w=CARD_W;let card_h=CARD_H;let card_spacing=CARD_GAP;
                            let total_w=count as f64*card_w+(count.saturating_sub(1)) as f64*card_spacing;
                            let start_x=if count<=4 {(EXP_W-total_w)/2.} else {24.-s.shelf_scroll};
                            let mut removed_idx=None; let mut clicked_idx=None;
                            for idx in 0..count {
                                let card_x=start_x+idx as f64*(card_w+card_spacing);let card_y=CARD_Y;
                                let rem_dx=x-(card_x+CLOSE_DX);let rem_dy=y-(card_y+CLOSE_DY);
                                if rem_dx*rem_dx+rem_dy*rem_dy<=100. {removed_idx=Some(idx);break;}
                                if x>=card_x && x<=card_x+card_w && y>=card_y && y<=card_y+card_h {clicked_idx=Some(idx);break;}
                            }
                            if let Some(child_idx)=removed_idx {
                                s.remove_folder_child(folder_idx,child_idx);
                                InvalidateRect(hwnd,std::ptr::null(),0);
                            } else if let Some(child_idx)=clicked_idx {
                                open_file(&s.shelf_items[folder_idx].children[child_idx].path);
                            }
                        } else {
                            s.active_folder=None;
                            InvalidateRect(hwnd,std::ptr::null(),0);
                        }
                    } else {
                        let count=s.shelf_items.len();let card_w=CARD_W;let card_h=CARD_H;let card_spacing=CARD_GAP;
                        let total_w=count as f64*card_w+(count.saturating_sub(1)) as f64*card_spacing;
                        let start_x=if count<=4 {(EXP_W-total_w)/2.} else {24.-s.shelf_scroll};
                        let mut removed_idx=None; let mut clicked_idx=None;
                        for idx in 0..count {
                            let card_x=start_x+idx as f64*(card_w+card_spacing);let card_y=CARD_Y;
                            let rem_dx=x-(card_x+CLOSE_DX);let rem_dy=y-(card_y+CLOSE_DY);
                            if rem_dx*rem_dx+rem_dy*rem_dy<=100. {removed_idx=Some(idx);break;}
                            if x>=card_x && x<=card_x+card_w && y>=card_y && y<=card_y+card_h {clicked_idx=Some(idx);break;}
                        }
                        if let Some(idx)=removed_idx {
                            s.shelf_items.remove(idx);
                            if s.shelf_items.len()<=4 {s.shelf_scroll=0.;}
                            s.shelf_hover=-1;s.shelf_remove_hover=-1;
                            InvalidateRect(hwnd,std::ptr::null(),0);
                        } else if let Some(idx)=clicked_idx {
                            if s.shelf_items[idx].is_group() {
                                s.active_folder=Some(idx);
                                s.shelf_scroll=0.;
                                s.shelf_hover=-1;
                                s.shelf_remove_hover=-1;
                                InvalidateRect(hwnd,std::ptr::null(),0);
                            } else {
                                open_file(&s.shelf_items[idx].path);
                            }
                        }
                    }
                }
                return 0;
            }
            0x205=>{
                let x=(l as i16) as f64/s.scale;let y=((l>>16) as i16) as f64/s.scale;
                if s.usable() && s.active_tab==TAB_SHELF && y>=50. {
                    if let Some(folder_idx)=s.active_folder {
                        if folder_idx<s.shelf_items.len() && s.shelf_items[folder_idx].is_group() {
                            let count=s.shelf_items[folder_idx].children.len();
                            let card_w=CARD_W;let card_h=CARD_H;let card_spacing=CARD_GAP;
                            let total_w=count as f64*card_w+(count.saturating_sub(1)) as f64*card_spacing;
                            let start_x=if count<=4 {(EXP_W-total_w)/2.} else {24.-s.shelf_scroll};
                            for idx in 0..count {
                                let card_x=start_x+idx as f64*(card_w+card_spacing);let card_y=CARD_Y;
                                if x>=card_x && x<=card_x+card_w && y>=card_y && y<=card_y+card_h {
                                    open_in_explorer(&s.shelf_items[folder_idx].children[idx].path);
                                    return 0;
                                }
                            }
                        }
                    } else if !s.shelf_items.is_empty() {
                        let count=s.shelf_items.len();let card_w=CARD_W;let card_h=CARD_H;let card_spacing=CARD_GAP;
                        let total_w=count as f64*card_w+(count.saturating_sub(1)) as f64*card_spacing;
                        let start_x=if count<=4 {(EXP_W-total_w)/2.} else {24.-s.shelf_scroll};
                        for idx in 0..count {
                            let card_x=start_x+idx as f64*(card_w+card_spacing);let card_y=CARD_Y;
                            if x>=card_x && x<=card_x+card_w && y>=card_y && y<=card_y+card_h {
                                if s.shelf_items[idx].is_group() {
                                    if let Some(first)=s.shelf_items[idx].children.first() {
                                        open_in_explorer(&first.path);
                                    }
                                } else {
                                    open_in_explorer(&s.shelf_items[idx].path);
                                }
                                return 0;
                            }
                        }
                    }
                }
                return 0;
            }
            0x020a|0x020e=>{
                let delta=(w as i32 >> 16) as i16 as f64;
                let mut r=Rect::default();GetWindowRect(hwnd,&mut r);
                let y=(((l>>16) as i16) as i32-r.top) as f64/s.scale;
                let count=s.current_items().len();
                // Wheel over the volume control changes the volume.
                if msg==0x020a && s.usable() && s.active_tab==TAB_MUSIC && s.vol_level>=0. {
                    let x=(((l as i16) as i32-r.left) as f64)/s.scale;
                    if x>=SPEAKER_X-16. && (y-(s.panel_h-26.)).abs()<=16. {
                        let level=(s.vol_level+(delta/120.) as f32*0.04).clamp(0.,1.);
                        s.volume.set_level(level);s.vol_level=level;
                        if level>0. {s.vol_muted=false;}
                        InvalidateRect(hwnd,std::ptr::null(),0);
                        return 0;
                    }
                }
                if msg==0x020a && s.usable() && s.active_tab==TAB_TODO && s.todos.len()>TODO_ROWS && y>=ROW_Y0 {
                    s.todo_scroll=if delta<0. {s.todo_scroll+1} else {s.todo_scroll.saturating_sub(1)};
                    s.clamp_todo_scroll();
                    InvalidateRect(hwnd,std::ptr::null(),0);
                    return 0;
                }
                if msg==0x020a && s.usable() && s.active_tab==TAB_SHELF && count>4 && y>=CARD_Y {
                    let total_w=count as f64*CARD_W+(count-1) as f64*CARD_GAP;
                    let max_scroll=(total_w-(EXP_W-48.)).max(0.);
                    s.shelf_scroll=(s.shelf_scroll-delta*0.5).clamp(0.,max_scroll);
                    InvalidateRect(hwnd,std::ptr::null(),0);
                    return 0;
                }
                if s.expanded && s.progress>0.5 {
                    // Wheel down / swipe right goes to the next page. One page per gesture: input is
                    // dropped while a switch is still landing, and touchpad deltas accumulate to a notch.
                    let step=if msg==0x020e {delta} else {-delta};
                    let settling=s.tab_to!=s.active_tab || s.tab_t<0.6;
                    if settling || step.signum()!=s.wheel_acc.signum() {s.wheel_acc=0.;}
                    if !settling {
                        s.wheel_acc+=step;
                        if s.wheel_acc.abs()>=120. {
                            let next=if s.wheel_acc>0. {(s.active_tab+1).min(TAB_COUNT-1)} else {s.active_tab.saturating_sub(1)};
                            s.wheel_acc=0.;
                            if next!=s.active_tab {s.active_tab=next;InvalidateRect(hwnd,std::ptr::null(),0);}
                        }
                    }
                    return 0;
                }
            }
            0x0233=>{
                let hdrop=w as Handle;
                let count=DragQueryFileW(hdrop,0xFFFFFFFF,null_mut(),0);
                let mut paths=Vec::with_capacity(count as usize);
                for i in 0..count {
                    let len=DragQueryFileW(hdrop,i,null_mut(),0);
                    let mut buf=vec![0u16;(len+1) as usize];
                    DragQueryFileW(hdrop,i,buf.as_mut_ptr(),len+1);
                    buf.pop();
                    paths.push(String::from_utf16_lossy(&buf));
                }
                DragFinish(hdrop);
                s.add_shelf_items(&paths,hwnd);
                return 0;
            }
            0x8004=>{
                if s.dragging_out {return 0;}
                s.expanded=true;
                s.active_tab=TAB_SHELF;
                s.ole_drag_target=None;
                s.layout(hwnd);
                InvalidateRect(hwnd,std::ptr::null(),0);
                return 0;
            }
            0x8005=>{
                if s.dragging_out {return 0;}
                s.expanded=false;
                s.hover=-1;s.pressed=-1;s.shelf_hover=-1;s.shelf_remove_hover=-1;s.tab_hover=-1;s.add_button_hover=false;
                s.breadcrumb_hover=false;s.drag_source=None;s.drag_target=None;s.is_dragging=false;s.ole_drag_target=None;
                s.layout(hwnd);
                InvalidateRect(hwnd,std::ptr::null(),0);
                return 0;
            }
            0x8006=>{
                if s.dragging_out {return 0;}
                let paths=&*(w as *const Vec<String>);
                s.add_shelf_items(paths,hwnd);
                s.expanded=true;
                s.active_tab=TAB_SHELF;
                s.ole_drag_target=None;
                s.layout(hwnd);
                InvalidateRect(hwnd,std::ptr::null(),0);
                return 0;
            }
            0x8007=>{
                if s.dragging_out {return 0;}
                let screen_x=w as i32;
                let screen_y=l as i32;
                let mut r=Rect::default();
                GetWindowRect(hwnd,&mut r);
                let x=(screen_x-r.left) as f64/s.scale;
                let y=(screen_y-r.top) as f64/s.scale;
                s.update_ole_drag_hover(x,y);
                InvalidateRect(hwnd,std::ptr::null(),0);
                return 0;
            }
            0x102 if s.todo_editing=>{
                let c=w as u16;
                match c {
                    8=>{s.todo_input.pop();}
                    13=>s.commit_todo(),
                    0xD800..=0xDBFF=>s.high_surrogate=c,
                    0xDC00..=0xDFFF=>{
                        if let Some(Ok(ch))=char::decode_utf16([s.high_surrogate,c]).next() {s.push_input(ch);}
                        s.high_surrogate=0;
                    }
                    c if c>=32 && c!=127=>{if let Some(ch)=char::from_u32(c as u32) {s.push_input(ch);}}
                    _=>{}
                }
                InvalidateRect(hwnd,std::ptr::null(),0);
                return 0;
            }
            0x0008=>{
                // Focus moved to another app: stop typing, and close if the cursor is elsewhere.
                if s.todo_editing {
                    s.todo_editing=false;
                    let mut r=Rect::default();GetWindowRect(hwnd,&mut r);let mut c=Point::default();GetCursorPos(&mut c);
                    if !s.smoke && !(c.x>=r.left && c.x<r.right && c.y>=r.top && c.y<r.bottom) {s.expanded=false;s.layout(hwnd);}
                    InvalidateRect(hwnd,std::ptr::null(),0);
                }
            }
            0x100 if w==27=>{
                if s.todo_editing {s.todo_editing=false;InvalidateRect(hwnd,std::ptr::null(),0);return 0;}
                if s.active_folder.is_some() {
                    s.active_folder=None;
                    s.shelf_scroll=0.;
                    s.shelf_hover=-1;
                    s.shelf_remove_hover=-1;
                    InvalidateRect(hwnd,std::ptr::null(),0);
                    return 0;
                }
                s.expanded=false;
                s.hover=-1;s.pressed=-1;s.shelf_hover=-1;s.shelf_remove_hover=-1;s.tab_hover=-1;s.add_button_hover=false;
                s.breadcrumb_hover=false;s.drag_source=None;s.drag_target=None;s.is_dragging=false;
                InvalidateRect(hwnd,std::ptr::null(),0);
                return 0;
            }
            0x1a=>{
                s.reduced_motion=s.settings.follow_windows_animations && reduced_motion();
                if let Some(tray)=&mut s.tray {tray.refresh_theme();}
                return 0;
            }
            tray::CALLBACK=>{
                let chosen=match &s.tray {
                    Some(tray)=>{s.menu_active=true;let chosen=tray.handle(l);s.menu_active=false;chosen}
                    None=>0,
                };
                match chosen {
                    tray::item::QUIT=>{DestroyWindow(hwnd);}
                    tray::item::SETTINGS=>{
                        // Open straight onto the Settings page; like the Pomodoro alert, it closes again after a
                        // while unless the cursor is on it.
                        s.autostart=settings::starts_with_windows();
                        s.active_tab=TAB_SETTINGS;s.active_folder=None;
                        if !s.expanded {s.expanded=true;s.layout(hwnd);}
                        s.alert_until=Some(std::time::Instant::now()+std::time::Duration::from_secs(8));
                        s.pace.store(PACE_MOTION,std::sync::atomic::Ordering::Relaxed);
                        InvalidateRect(hwnd,std::ptr::null(),0);
                    }
                    _=>{}
                }
                return 0;
            }
            m if s.tray.as_ref().is_some_and(|t|t.is_taskbar_created(m))=>{
                if let Some(tray)=&s.tray {tray.add();}
                return 0;
            }
            0x7e|0x2e0=>{s.layout(hwnd);return 0;}
            2=>{
                s.tray=None;
                RevokeDragDrop(hwnd);
                OleUninitialize();
                PostQuitMessage(0);
                return 0;
            }
            _=>{}
        }
    }
    DefWindowProcW(hwnd,msg,w,l)
}
/// Frame-thread pace, chosen by the UI thread at the end of each tick.
const PACE_IDLE:u8=0; const PACE_AMBIENT:u8=1; const PACE_MOTION:u8=2;
#[link(name="user32")]
extern "system" {fn GetDC(hwnd:Hwnd)->Handle; fn ReleaseDC(hwnd:Hwnd,dc:Handle)->i32;}
#[link(name="gdi32")]
extern "system" {fn GetDeviceCaps(dc:Handle,index:i32)->i32;}
#[link(name="user32")]
extern "system" {
    fn GetTopWindow(hwnd:Hwnd)->Hwnd;
    fn GetWindow(hwnd:Hwnd,cmd:u32)->Hwnd;
    fn IsWindowVisible(hwnd:Hwnd)->i32;
    fn IsIconic(hwnd:Hwnd)->i32;
    fn GetClassNameW(hwnd:Hwnd,name:*mut u16,count:i32)->i32;
}
/// Windows hidden by DWM (other virtual desktops, suspended Store apps) still report as visible.
unsafe fn cloaked(hwnd:Hwnd)->bool {
    static GET:std::sync::LazyLock<usize>=std::sync::LazyLock::new(|| unsafe {
        let module=LoadLibraryW(wide("dwmapi.dll").as_ptr());
        if module.is_null() {0} else {GetProcAddress(module,b"DwmGetWindowAttribute\0".as_ptr()) as usize}
    });
    if *GET==0 {return false;}
    let get:unsafe extern "system" fn(Hwnd,u32,*mut c_void,u32)->i32=std::mem::transmute(*GET);
    let mut value=0u32;
    get(hwnd,14,(&mut value as *mut u32).cast(),4)==0 && value!=0
}
/// True when the topmost real window under `footprint` (screen pixels) is an app window rather than
/// the desktop: something would sit behind the closed notch. Click-through overlays are ignored.
unsafe fn covers_window(own:Hwnd,footprint:Rect)->bool {
    let mut w=GetTopWindow(null_mut());
    while !w.is_null() {
        if w!=own && IsWindowVisible(w)!=0 && IsIconic(w)==0 && GetWindowLongPtrW(w,-20) as u32&0x20==0 && !cloaked(w) {
            let mut r=Rect::default();GetWindowRect(w,&mut r);
            if r.right>r.left && r.bottom>r.top && r.left<footprint.right && r.right>footprint.left && r.top<footprint.bottom && r.bottom>footprint.top {
                let mut name=[0u16;64];
                let len=GetClassNameW(w,name.as_mut_ptr(),name.len() as i32).max(0) as usize;
                let class=String::from_utf16_lossy(&name[..len]);
                return class!="Progman" && class!="WorkerW";
            }
        }
        w=GetWindow(w,2);
    }
    false
}
#[link(name="gdi32")]
extern "system" {fn BitBlt(dst:Handle,x:i32,y:i32,w:i32,h:i32,src:Handle,sx:i32,sy:i32,rop:u32)->i32;}
/// True when the screen pixels in `areas` (screen pixels) are all one flat color, give or take
/// antialiasing noise: nothing but an empty title bar or tab strip.
unsafe fn screen_is_flat(areas:&[Rect])->bool {
    // Each area is judged on its own: pooling them would let a few letters in a small strip vanish
    // into a big empty area, so the notch would shrink while normal and grow back once shrunk.
    let screen=GetDC(null_mut());
    let mut flat=true;
    for a in areas {
        let mut pixels:Vec<u32>=Vec::new();
        let (w,h)=(a.right-a.left,a.bottom-a.top);
        if w<=0 || h<=0 {continue;}
        let dc=CreateCompatibleDC(screen);
        let info=BitmapInfo{size:40,width:w,height:-h,planes:1,bits:32,compression:0,bytes:0,x:0,y:0,used:0,important:0};
        let mut bits=null_mut();
        let bitmap=CreateDIBSection(dc,&info,0,&mut bits,null_mut(),0);
        if !bitmap.is_null() && !bits.is_null() {
            let old=SelectObject(dc,bitmap);
            BitBlt(dc,0,0,w,h,screen,a.left,a.top,0x00CC0020);
            GdiFlush();
            pixels.extend_from_slice(std::slice::from_raw_parts(bits as *const u32,(w*h) as usize));
            SelectObject(dc,old);DeleteObject(bitmap);
        }
        DeleteDC(dc);
        if !is_flat(&pixels) {flat=false;break;}
    }
    ReleaseDC(null_mut(),screen);
    flat
}
/// Minimal-mode decision from one observation. Shrinking needs content seen twice (~0.5 s); growing
/// back needs a clear area eight times in a row (~2 s); and after any switch the opposite one waits
/// 3 s, so nothing near the notch's edge can make it ping-pong.
fn cover_step(covered:bool,pending:&mut bool,count:&mut u8,since_switch:f64,observed:bool)->bool {
    if observed==*pending {*count=count.saturating_add(1);} else {*pending=observed;*count=1;}
    let needed=if observed {2} else {8};
    if observed!=covered && *count>=needed && since_switch>=3. {observed} else {covered}
}
/// At least 97% of the pixels within a small distance of the most common color.
fn is_flat(pixels:&[u32])->bool {
    if pixels.is_empty() {return true;}
    let mut counts=std::collections::HashMap::new();
    for &p in pixels {*counts.entry(p&0xf0f0f0).or_insert(0u32)+=1;}
    let common=counts.into_iter().max_by_key(|&(_,n)|n).map_or(0,|(c,_)|c);
    let near=|p:u32|{let d=|s:u32|((p>>s)&0xf0).abs_diff((common>>s)&0xf0);d(16)+d(8)+d(0)<=48};
    pixels.iter().filter(|&&p|near(p)).count()*100>=pixels.len()*97
}
/// One frame at the primary display's refresh rate (60 Hz if Windows does not say).
fn refresh_interval()->std::time::Duration {
    let hz=unsafe {let dc=GetDC(null_mut());let hz=GetDeviceCaps(dc,116);ReleaseDC(null_mut(),dc);hz};
    std::time::Duration::from_secs_f64(1./if (24..=500).contains(&hz) {hz as f64} else {60.})
}
static INTER_REGULAR:&[u8]=include_bytes!("../assets/fonts/Inter-Regular.ttf");
static INTER_MEDIUM:&[u8]=include_bytes!("../assets/fonts/Inter-Medium.ttf");
/// Face name per weight: SF Pro Display if installed, else the bundled Inter (its Medium weight is its
/// own family, "Inter Medium"), else Segoe UI if Inter could not be loaded.
unsafe fn font_faces()->fn(i32)->&'static str {
    if font_installed("SF Pro Display") {return |_| "SF Pro Display";}
    let mut loaded=true;
    for data in [INTER_REGULAR,INTER_MEDIUM] {
        let mut count=0u32;
        // Private to this process: nothing is installed, and the fonts go away when Wisp exits.
        if AddFontMemResourceEx(data.as_ptr().cast(),data.len() as u32,null_mut(),&mut count).is_null() || count==0 {loaded=false;}
    }
    if loaded && font_installed("Inter") && font_installed("Inter Medium") {
        |weight| if weight>=500 {"Inter Medium"} else {"Inter"}
    } else {
        |_| "Segoe UI"
    }
}
/// True when GDI actually gives `face` (rather than a substitute) for a font of that name.
unsafe fn font_installed(face:&str)->bool {
    let name=wide(face);
    let font=CreateFontW(-16,0,0,0,400,0,0,0,1,0,0,4,0,name.as_ptr());
    let dc=CreateCompatibleDC(null_mut());
    let old=SelectObject(dc,font);
    let mut buffer=[0u16;64];
    let len=GetTextFaceW(dc,buffer.len() as i32,buffer.as_mut_ptr()).max(1) as usize-1;
    SelectObject(dc,old);DeleteObject(font);DeleteDC(dc);
    String::from_utf16_lossy(&buffer[..len.min(buffer.len())]).eq_ignore_ascii_case(face)
}
fn main() {
    unsafe {
        OleInitialize(null_mut());
        SetProcessDPIAware();
        let instance=GetModuleHandleW(std::ptr::null()); let class=wide("WispNotch");
        let wc=WindowClass{size:size_of::<WindowClass>() as u32,style:3,proc:procedure,cls_extra:0,wnd_extra:0,instance,icon:null_mut(),cursor:LoadCursorW(null_mut(),32512usize as *const u16),background:null_mut(),menu:std::ptr::null(),name:class.as_ptr(),small_icon:null_mut()};
        if RegisterClassExW(&wc)==0 {OleUninitialize();return;}
        let hwnd=CreateWindowExW(0x80000|0x80|0x8,class.as_ptr(),wide("Wisp").as_ptr(),0x80000000,0,0,248,42,null_mut(),null_mut(),instance,null_mut());
        if hwnd.is_null() {OleUninitialize();return;}
        DragAcceptFiles(hwnd,1);
        ChangeWindowMessageFilter(0x0233,1);
        ChangeWindowMessageFilter(0x0049,1);
        ChangeWindowMessageFilter(0x004A,1);
        let drop_target=Box::into_raw(Box::new(DropTarget{lp_vtbl:&DROP_TARGET_VTBL,hwnd}));
        RegisterDragDrop(hwnd,drop_target as *mut c_void);
        let mut state=Box::new(State::new(hwnd,std::env::args().any(|a|a=="--smoke-test"),drop_target));
        state.apply_settings();
        state.snap_switches();
        SetWindowLongPtrW(hwnd,-21,(&mut *state) as *mut State as isize);state.layout(hwnd);ShowWindow(hwnd,4);
        let running=state.running.clone();let pace=state.pace.clone();let window=hwnd as usize;
        std::thread::spawn(move || {
            let library=LoadLibraryW(wide("dwmapi.dll").as_ptr());let address=GetProcAddress(library,b"DwmFlush\0".as_ptr());
            if address.is_null(){return;}
            let flush:unsafe extern "system" fn()->i32=std::mem::transmute(address);
            let mut last=std::time::Instant::now();
            let mut frame=refresh_interval();let mut checked=std::time::Instant::now();
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                if checked.elapsed()>std::time::Duration::from_secs(5) {frame=refresh_interval();checked=std::time::Instant::now();}
                match pace.load(std::sync::atomic::Ordering::Relaxed) {
                    PACE_MOTION=>{
                        // DwmFlush waits for the next composition, so springs land on every vsync. When the display
                        // is off or the session is locked it can return at once (or fail); then fall back to the
                        // timer so the UI thread is never flooded and WM_PAINT is never starved.
                        let waited=flush()>=0;
                        let elapsed=last.elapsed();
                        if !waited || elapsed<std::time::Duration::from_millis(4) {std::thread::sleep(frame.saturating_sub(elapsed));}
                    }
                    PACE_AMBIENT=>{
                        // Waveform and lyric glide: the display rate from a high-resolution timer. DwmFlush
                        // costs noticeable CPU on some drivers, and these small motions do not need vsync lock.
                        std::thread::sleep(frame.saturating_sub(last.elapsed()));
                    }
                    _=>{
                        // Idle: ~10 ticks a second, checking every 10 ms so input wakes it straight away.
                        while last.elapsed()<std::time::Duration::from_millis(100) && pace.load(std::sync::atomic::Ordering::Relaxed)==PACE_IDLE {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                    }
                }
                last=std::time::Instant::now();
                if !running.load(std::sync::atomic::Ordering::Relaxed){break;}
                SendMessageW(window as Hwnd,0x8001,0,0);
            }
        });
        let mut msg:Message=std::mem::zeroed();while GetMessageW(&mut msg,null_mut(),0,0)>0 {TranslateMessage(&msg);DispatchMessageW(&msg);}
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_data_object_carries_paths() {
        unsafe { OleInitialize(null_mut()); }
        let dir = std::env::temp_dir().join("wisp-dragout-test");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let a = dir.join("a.txt");
        let b = sub.join("b.txt");
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        let paths = vec![a.to_string_lossy().to_string(), b.to_string_lossy().to_string()];
        let data = unsafe { file_data_object(&paths) };
        assert!(!data.is_null(), "SHCreateDataObject failed");
        let got = unsafe { extract_hdrop_paths(data as *mut IDataObject) };
        unsafe { ((*(*(data as *mut IDataObject)).lp_vtbl).release)(data); }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got, paths);
    }

    #[test]
    fn test_cover_step_does_not_ping_pong() {
        let (mut pending, mut count) = (false, 0u8);
        // Content seen twice shrinks it (well after start-up).
        assert!(!cover_step(false, &mut pending, &mut count, 10., true));
        assert!(cover_step(false, &mut pending, &mut count, 10., true));
        // Clear checks only grow it back after eight in a row, and not within 3 s of the last switch.
        let mut covered = true;
        for i in 0..7 {
            covered = cover_step(covered, &mut pending, &mut count, 5. + i as f64, false);
            assert!(covered, "grew back after {} clear checks", i + 1);
        }
        assert!(cover_step(true, &mut pending, &mut count, 1., false), "cooldown ignored");
        assert!(!cover_step(true, &mut pending, &mut count, 3.5, false));
        // Alternating observations never switch it.
        let (mut pending, mut count, mut covered) = (true, 2u8, true);
        for i in 0..40 {
            covered = cover_step(covered, &mut pending, &mut count, 10., i % 2 == 0);
            assert!(covered);
        }
    }

    #[test]
    fn test_flat_detection() {
        let bar = vec![0xf0f0f0u32; 1000];
        assert!(is_flat(&bar));
        // Antialiasing noise and a faint gradient still count as empty.
        let noisy: Vec<u32> = (0..1000).map(|i| if i % 50 == 0 { 0xe8e8e8 } else { 0xf2f2f2 }).collect();
        assert!(is_flat(&noisy));
        // Tab text or icons: a few percent of clearly different pixels.
        let tabs: Vec<u32> = (0..1000).map(|i| if i % 20 < 2 { 0x202020 } else { 0xf0f0f0 }).collect();
        assert!(!is_flat(&tabs));
        // One side a tab, the other the empty strip: two different flat colors.
        let split: Vec<u32> = (0..1000).map(|i| if i < 500 { 0x3c3c3c } else { 0xdedede }).collect();
        assert!(!is_flat(&split));
    }

    #[test]
    fn test_font_installed_detects_substitution() {
        unsafe {
            assert!(font_installed("Segoe UI"));
            assert!(!font_installed("Definitely Not A Real Font 7f3a"));
        }
    }

    #[test]
    fn test_bundled_inter_loads() {
        unsafe {
            let face = font_faces();
            if !font_installed("SF Pro Display") {
                assert_eq!((face(400), face(500)), ("Inter", "Inter Medium"));
            }
            // Either way the bundled files register privately and resolve by name.
            assert!(font_installed("Inter") && font_installed("Inter Medium"));
        }
    }

    #[test]
    fn test_shelf_round_trip() {
        let items = vec![
            create_shelf_item("C:\\a\\one.txt"),
            create_folder_item(vec![create_shelf_item("C:\\b\\two.png"), create_shelf_item("D:\\three.zip")]),
        ];
        let parsed = parse_shelf(&serialize_shelf(&items));
        assert_eq!(parsed, vec![vec!["C:\\a\\one.txt".to_string()], vec!["C:\\b\\two.png".to_string(), "D:\\three.zip".to_string()]]);
        assert!(parse_shelf("X\tC:\\x\nF\t\nG\n").is_empty());
    }

    #[test]
    fn test_com_struct_layouts() {
        assert_eq!(std::mem::size_of::<PointL>(), 8);
        assert_eq!(std::mem::size_of::<Guid>(), 16);
        assert_eq!(std::mem::size_of::<FormatEtc>(), 32);
        assert_eq!(std::mem::size_of::<StgMedium>(), 24);
        assert_eq!(std::mem::size_of::<DropTarget>(), 16);
    }

    #[test]
    fn test_guid_values() {
        assert_eq!(IID_IUNKNOWN.data1, 0);
        assert_eq!(IID_IUNKNOWN.data4, [0xc0, 0, 0, 0, 0, 0, 0, 0x46]);
        assert_eq!(IID_IDROPTARGET.data1, 0x00000122);
        assert_eq!(IID_IDROPTARGET.data4, [0xc0, 0, 0, 0, 0, 0, 0, 0x46]);
    }

    #[test]
    fn test_drop_target_query_interface() {
        let mut target = DropTarget { lp_vtbl: &DROP_TARGET_VTBL, hwnd: null_mut() };
        let mut ppv = null_mut();
        unsafe {
            let hr = dt_query_interface(&mut target, &IID_IDROPTARGET, &mut ppv);
            assert_eq!(hr, 0);
            assert_eq!(ppv, &mut target as *mut DropTarget as *mut c_void);

            ppv = null_mut();
            let hr2 = dt_query_interface(&mut target, &IID_IUNKNOWN, &mut ppv);
            assert_eq!(hr2, 0);
            assert_eq!(ppv, &mut target as *mut DropTarget as *mut c_void);

            ppv = null_mut();
            let bad_guid = Guid { data1: 1, data2: 2, data3: 3, data4: [0; 8] };
            let hr3 = dt_query_interface(&mut target, &bad_guid, &mut ppv);
            assert_ne!(hr3, 0);
            assert!(ppv.is_null());
        }
    }

    #[test]
    fn test_drop_target_ref_counting() {
        let mut target = DropTarget { lp_vtbl: &DROP_TARGET_VTBL, hwnd: null_mut() };
        unsafe {
            assert_eq!(dt_add_ref(&mut target), 1);
            assert_eq!(dt_release(&mut target), 1);
        }
    }

    #[test]
    fn test_folder_grouping_and_ungrouping() {
        let f1 = create_shelf_item("test_a.txt");
        let f2 = create_shelf_item("test_b.png");
        let f3 = create_shelf_item("test_c.rs");

        let items = vec![f1, f2, f3];
        let mut state = State {
            expanded: true,
            progress: 1.,
            velocity: 0.,
            play_progress: 0.,
            play_velocity: 0.,
            reduced_motion: false,
            scale: 1.0,
            fonts: [null_mut(); 8],
            ticks: 0,
            smoke: false,
            verified: false,
            smoke_shelf_verified: false,
            smoke_drag_verified: false,
            drop_target: null_mut(),
            media: media::Media::default(),
            receiver: std::sync::mpsc::channel().1,
            hover: -1,
            pressed: -1,
            clock: std::time::Instant::now(),
            last_frame: std::time::Instant::now(),
            dc: null_mut(),
            bitmap: null_mut(),
            original: null_mut(),
            pixels: null_mut(),
            stride: 720,
            text_buffer: std::cell::RefCell::new(Vec::new()),
            text_cache: Default::default(),
            icon_cache: Default::default(),
            renderer: None,
            running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            pace: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(PACE_MOTION)),
            menu_active: false,
            last_minute: 0,
            shelf_items: items,
            active_tab: TAB_SHELF,
            tab_from: TAB_SHELF,
            tab_to: TAB_SHELF,
            tab_t: 1.,
            tab_vel: 0.,
            panel_w: EXP_W,
            panel_vel: 0.,
            wheel_acc: 0.,
            todos: Vec::new(),
            todo_input: String::new(),
            todo_editing: false,
            todo_scroll: 0,
            high_surrogate: 0,
            pomo: tasks::Pomodoro::new(),
            timer_t: 0.,
            timer_vel: 0.,
            alert_until: None,
            last_pomo_sec: 0,
            lyrics_tx: None,
            lyrics_rx: None,
            lyrics_key: String::new(),
            lyrics: lyrics::Status::NotFound,
            lyrics_on: false,
            lyrics_t: 0.,
            lyrics_tv: 0.,
            lyric_scroll: -1.,
            lyric_vel: 0.,
            lyric_heights: std::cell::RefCell::new(Vec::new()),
            lyric_hits: std::cell::RefCell::new(Vec::new()),
            play_pos: 0.,
            panel_h: EXP_H,
            panel_hv: 0.,
            volume: volume::Volume::none(),
            vol_level: -1.,
            vol_muted: false,
            vol_drag: false,
            clyric_t: 0.,
            clyric_tv: 0.,
            lyric_w: COL_W,
            lyric_wv: 0.,
            lyric_w_target: COL_W,
            clyric_key: (-2, false),
            last_sample: -1.,
            dragging_out: false,
            shelf_saved: String::new(),
            tray: None,
            shelf_scroll: 0.,
            shelf_hover: -1,
            shelf_remove_hover: -1,
            tab_hover: -1,
            add_button_hover: false,
            active_folder: None,
            breadcrumb_hover: false,
            drag_source: None,
            drag_start_pos: (0., 0.),
            drag_current_pos: (0., 0.),
            is_dragging: false,
            drag_target: None,
            ole_drag_target: None,
            notify_rx: None,
            notices: std::collections::VecDeque::new(),
            notice: None,
            notice_until: None,
            notice_t: 0.,
            notice_v: 0.,
            notice_hover: false,
            autostart: false,
            switch_t: [0.; 7],
            switch_v: [0.; 7],
            seg_t: 0.,
            seg_v: 0.,
            mini_t: 0.,
            mini_v: 0.,
            covered: false,
            cover_pending: false,
            cover_count: 0,
            cover_checked: std::time::Instant::now(),
            cover_switched: std::time::Instant::now(),
            cover_w: 0.,
            settings: settings::Settings::default(),
        };

        // Merge file 1 into file 0 -> creates folder with 2 items
        state.merge_items(1, 0);
        assert_eq!(state.shelf_items.len(), 2);
        assert!(state.shelf_items[0].is_group());
        assert_eq!(state.shelf_items[0].name, "Folder");
        assert_eq!(state.shelf_items[0].size_str, "2 items");
        assert_eq!(state.shelf_items[0].children.len(), 2);
        assert_eq!(classify_file(&state.shelf_items[0]), FileKind::Folder);
        assert_eq!(state.total_items(), 3);

        // Merge 3rd file into existing folder -> folder now has 3 items
        state.merge_items(1, 0);
        assert_eq!(state.shelf_items.len(), 1);
        assert!(state.shelf_items[0].is_group());
        assert_eq!(state.shelf_items[0].children.len(), 3);
        assert_eq!(state.shelf_items[0].size_str, "3 items");

        // Enter folder view
        state.active_folder = Some(0);
        assert_eq!(state.current_items().len(), 3);

        // Remove 1 child from folder -> 2 items remain, stays in folder view
        state.remove_folder_child(0, 0);
        assert_eq!(state.shelf_items[0].children.len(), 2);
        assert_eq!(state.shelf_items[0].size_str, "2 items");
        assert_eq!(state.active_folder, Some(0));

        // Remove 1 more child -> 1 item remains -> ungroups cleanly!
        state.remove_folder_child(0, 0);
        assert_eq!(state.shelf_items.len(), 1);
        assert!(!state.shelf_items[0].is_group());
        assert_eq!(state.active_folder, None);
        assert_eq!(state.shelf_items[0].name, "test_c.rs");

        // Remove final item
        state.shelf_items.clear();
        assert_eq!(state.total_items(), 0);
    }
}

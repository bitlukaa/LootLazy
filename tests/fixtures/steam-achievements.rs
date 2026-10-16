// Original Steam flat-API fixture. No Steam client or user data is accessed.
use std::{ffi::{c_char,c_void,CStr},sync::atomic::{AtomicBool,AtomicI32,AtomicU32,Ordering::*}};
static APP:AtomicU32=AtomicU32::new(5);static NEXT:AtomicI32=AtomicI32::new(0);static A:AtomicBool=AtomicBool::new(false);static B:AtomicBool=AtomicBool::new(true);static mut PAYLOAD:[u8;24]=[0;24];
#[repr(C)]pub struct Callback{user:i32,id:i32,data:*mut c_void,size:i32}
#[no_mangle]pub extern "C" fn SteamAPI_Init()->bool{A.store(false,SeqCst);B.store(true,SeqCst);NEXT.store(0,SeqCst);true}
#[no_mangle]pub extern "C" fn SteamAPI_Shutdown(){}
#[no_mangle]pub extern "C" fn SteamAPI_ManualDispatch_Init(){}
#[no_mangle]pub extern "C" fn SteamAPI_GetHSteamPipe()->i32{1}
#[no_mangle]pub extern "C" fn SteamAPI_ManualDispatch_RunFrame(_:i32){}
#[no_mangle]pub extern "C" fn SteamAPI_ManualDispatch_FreeLastCallback(_:i32){}
#[no_mangle]pub extern "C" fn SteamAPI_SteamUserStats_v013()->*mut c_void{1usize as *mut c_void}
#[no_mangle]pub extern "C" fn SteamAPI_SteamUser_v023()->*mut c_void{1usize as *mut c_void}
#[no_mangle]pub extern "C" fn SteamAPI_SteamApps_v008()->*mut c_void{1usize as *mut c_void}
#[no_mangle]pub extern "C" fn SteamAPI_ISteamApps_BIsSubscribedApp(_:*mut c_void,app:u32)->bool{APP.store(app,SeqCst);true}
#[no_mangle]pub extern "C" fn SteamAPI_ISteamUser_GetSteamID(_:*mut c_void)->u64{7}
#[no_mangle]pub extern "C" fn SteamAPI_ISteamUserStats_RequestUserStats(_:*mut c_void,_:u64)->u64{NEXT.store(703,SeqCst);42}
#[no_mangle]pub unsafe extern "C" fn SteamAPI_ManualDispatch_GetNextCallback(_:i32,out:*mut Callback)->bool{let id=NEXT.swap(0,SeqCst);if id==0{return false;}let mut bytes=[0u8;24];if id==703{bytes[..8].copy_from_slice(&42u64.to_le_bytes());bytes[8..12].copy_from_slice(&1101i32.to_le_bytes());bytes[12..16].copy_from_slice(&24u32.to_le_bytes());}else{bytes[..8].copy_from_slice(&(APP.load(SeqCst) as u64).to_le_bytes());bytes[8..12].copy_from_slice(&(if APP.load(SeqCst)==6{2u32}else{1u32}).to_le_bytes());}std::ptr::copy_nonoverlapping(bytes.as_ptr(),std::ptr::addr_of_mut!(PAYLOAD).cast::<u8>(),24);*out=Callback{user:1,id,data:std::ptr::addr_of_mut!(PAYLOAD).cast(),size:if id==703{16}else{16}};true}
#[no_mangle]pub unsafe extern "C" fn SteamAPI_ManualDispatch_GetAPICallResult(_:i32,call:u64,out:*mut c_void,size:i32,id:i32,failed:*mut bool)->bool{if call!=42||id!=1101||size!=24{return false;}let mut data=[0u8;24];data[..8].copy_from_slice(&(APP.load(SeqCst) as u64).to_le_bytes());data[8..12].copy_from_slice(&1u32.to_le_bytes());data[16..24].copy_from_slice(&7u64.to_le_bytes());std::ptr::copy_nonoverlapping(data.as_ptr(),out.cast(),24);*failed=false;true}
#[no_mangle]pub extern "C" fn SteamAPI_ISteamUserStats_GetNumAchievements(_:*mut c_void)->u32{2}
#[no_mangle]pub extern "C" fn SteamAPI_ISteamUserStats_GetAchievementName(_:*mut c_void,index:u32)->*const c_char{if index==0{c"A".as_ptr()}else{c"B".as_ptr()}}
#[no_mangle]pub unsafe extern "C" fn SteamAPI_ISteamUserStats_GetAchievementDisplayAttribute(_:*mut c_void,key:*const c_char,attr:*const c_char)->*const c_char{if CStr::from_ptr(attr).to_bytes()==b"desc"{c"Fixture description".as_ptr()}else if CStr::from_ptr(key).to_bytes()==b"A"{c"Fixture A".as_ptr()}else{c"Fixture B".as_ptr()}}
#[no_mangle]pub unsafe extern "C" fn SteamAPI_ISteamUserStats_GetAchievementAndUnlockTime(_:*mut c_void,key:*const c_char,state:*mut bool,time:*mut u32)->bool{*state=if CStr::from_ptr(key).to_bytes()==b"A"{A.load(SeqCst)}else{B.load(SeqCst)};*time=0;true}
unsafe fn change(key:*const c_char,state:bool)->bool{if CStr::from_ptr(key).to_bytes()==b"A"{A.store(state,SeqCst);}else{B.store(state,SeqCst);}true}
#[no_mangle]pub unsafe extern "C" fn SteamAPI_ISteamUserStats_SetAchievement(_:*mut c_void,key:*const c_char)->bool{change(key,true)}
#[no_mangle]pub unsafe extern "C" fn SteamAPI_ISteamUserStats_ClearAchievement(_:*mut c_void,key:*const c_char)->bool{change(key,false)}
#[no_mangle]pub extern "C" fn SteamAPI_ISteamUserStats_StoreStats(_:*mut c_void)->bool{if APP.load(SeqCst)==9{return false;}NEXT.store(1102,SeqCst);true}

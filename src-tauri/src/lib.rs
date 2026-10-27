use image::{codecs::{gif::GifDecoder, jpeg::JpegEncoder, webp::WebPEncoder}, imageops::FilterType, AnimationDecoder, DynamicImage, GenericImageView, ImageEncoder, ImageFormat};
use serde::{Deserialize, Serialize};
use std::{fs, io::Cursor, path::{Path, PathBuf}};
use base64::{engine::general_purpose::STANDARD, Engine};

const REPOSITORY_URL: &str = "https://github.com/eeliyaa/Squeeze";
#[derive(Serialize)] struct Metadata { width: u32, height: u32, size: u64, animated: bool }
fn is_animated_gif(bytes: &[u8], format: ImageFormat) -> Result<bool, String> {
 if format != ImageFormat::Gif { return Ok(false); }
 let decoder=GifDecoder::new(Cursor::new(bytes)).map_err(|e|e.to_string())?;
 Ok(decoder.into_frames().take(2).count()>1)
}
#[tauri::command] fn inspect_image(path: String) -> Result<Metadata, String> {
 let bytes=fs::read(&path).map_err(|e|e.to_string())?;
 let format=image::guess_format(&bytes).map_err(|e|e.to_string())?;
 let animated=is_animated_gif(&bytes,format)?;
 let im=image::load_from_memory(&bytes).map_err(|e|e.to_string())?;
 let (width,height)=im.dimensions(); let size=bytes.len() as u64; Ok(Metadata{width,height,size,animated})
}
#[tauri::command] fn preview_image(path:String)->Result<String,String>{let im=image::open(path).map_err(|e|e.to_string())?.thumbnail(1000,700);let mut bytes=Cursor::new(Vec::new());im.write_to(&mut bytes,ImageFormat::Png).map_err(|e|e.to_string())?;Ok(format!("data:image/png;base64,{}",STANDARD.encode(bytes.into_inner())))}

#[derive(Deserialize)] #[serde(rename_all="camelCase")] struct Crop { x:u32, y:u32, width:u32, height:u32 }
#[derive(Deserialize)] #[serde(rename_all="camelCase")] struct ProcessRequest { path:String, output_dir:Option<String>, format:String, convert_format:bool, compress:bool, quality:u8, target_bytes:Option<u64>, width:Option<u32>, height:Option<u32>, keep_ratio:bool, flip_h:bool, flip_v:bool, rotation:i32, crop:Option<Crop> }
#[derive(Serialize)] #[serde(rename_all="camelCase")] struct ProcessResult { path:String, size:u64, target_met:Option<bool>, message:Option<String> }

fn quantized_rgba(im:&DynamicImage,quality:u8)->image::RgbaImage{let mut rgba=im.to_rgba8();let bits=5+((quality.saturating_sub(10) as u16*3)/90) as u8;let shift=8-bits;if shift>0{for pixel in rgba.pixels_mut(){for channel in &mut pixel.0[..3]{*channel=((*channel>>shift)<<shift)+(1<<(shift-1));}}}rgba}
fn encode(im:&DynamicImage,fmt:&str,quality:u8,compress_mode:bool)->Result<Vec<u8>,String>{
 let mut out=Cursor::new(Vec::new());
 match fmt {
  "jpg"=>JpegEncoder::new_with_quality(&mut out,quality.clamp(1,100)).encode_image(im).map_err(|e|e.to_string())?,
  "png"=>{if compress_mode{DynamicImage::ImageRgba8(quantized_rgba(im,quality)).write_to(&mut out,ImageFormat::Png)}else{im.write_to(&mut out,ImageFormat::Png)}.map_err(|e|e.to_string())?;},
  "webp"=>{let rgba=if compress_mode{quantized_rgba(im,quality)}else{im.to_rgba8()};WebPEncoder::new_lossless(&mut out).write_image(rgba.as_raw(),rgba.width(),rgba.height(),image::ExtendedColorType::Rgba8).map_err(|e|e.to_string())?;},
  "bmp"=>{let image=if compress_mode{DynamicImage::ImageRgba8(quantized_rgba(im,quality))}else{im.clone()};image.write_to(&mut out,ImageFormat::Bmp).map_err(|e|e.to_string())?;},
  "gif"=>{let image=if compress_mode{DynamicImage::ImageRgba8(quantized_rgba(im,quality))}else{im.clone()};image.write_to(&mut out,ImageFormat::Gif).map_err(|e|e.to_string())?;},
  "ico"=>{let image=if compress_mode{DynamicImage::ImageRgba8(quantized_rgba(im,quality))}else{im.clone()};let image=if image.width()>256||image.height()>256{image.resize(256,256,FilterType::Lanczos3)}else{image};image.write_to(&mut out,ImageFormat::Ico).map_err(|e|e.to_string())?;},
  "tiff"=>{let image=if compress_mode{DynamicImage::ImageRgba8(quantized_rgba(im,quality))}else{im.clone()};image.write_to(&mut out,ImageFormat::Tiff).map_err(|e|e.to_string())?;},
  _=>return Err("Unsupported output format".into())
 }
 Ok(out.into_inner())
}

#[tauri::command] fn process_image(request:ProcessRequest)->Result<ProcessResult,String>{
 let has_edit=request.width.is_some()||request.height.is_some()||request.flip_h||request.flip_v||request.rotation.rem_euclid(360)!=0||request.crop.is_some();if !request.convert_format&&!request.compress&&!has_edit{return Err("Select at least one operation.".into())}let src=PathBuf::from(&request.path);let source_bytes=fs::read(&src).map_err(|e|format!("Could not read image: {e}"))?;let source_format=image::guess_format(&source_bytes).map_err(|e|format!("Could not detect image format: {e}"))?;if is_animated_gif(&source_bytes,source_format)?{return Err("Animated GIFs cannot be edited without losing frames. They were not processed.".into())}let original_fmt=match source_format{ImageFormat::Jpeg=>"jpg",ImageFormat::Png=>"png",ImageFormat::WebP=>"webp",ImageFormat::Bmp=>"bmp",ImageFormat::Gif=>"gif",ImageFormat::Ico=>"ico",ImageFormat::Tiff=>"tiff",_=>return Err("This image format is not supported.".into())};let mut im=image::load_from_memory(&source_bytes).map_err(|e|format!("Could not open image: {e}"))?;
 if let Some(c)=request.crop { let x=c.x.min(im.width().saturating_sub(1));let y=c.y.min(im.height().saturating_sub(1));let w=c.width.max(1).min(im.width()-x);let h=c.height.max(1).min(im.height()-y);im=im.crop_imm(x,y,w,h); }
 if request.flip_h {im=im.fliph()} if request.flip_v {im=im.flipv()} im=match request.rotation.rem_euclid(360){90=>im.rotate90(),180=>im.rotate180(),270=>im.rotate270(),_=>im};
 if request.width.is_some()||request.height.is_some(){let (ow,oh)=im.dimensions();let (nw,nh)=match(request.width,request.height,request.keep_ratio){(Some(w),Some(h),false)=>(w,h),(Some(w),Some(_),true)=>(w,((oh as f64*w as f64/ow as f64).round() as u32).max(1)),(Some(w),None,_)=>(w,((oh as f64*w as f64/ow as f64).round() as u32).max(1)),(None,Some(h),_)=>( ((ow as f64*h as f64/oh as f64).round() as u32).max(1),h),_=>(ow,oh)};if nw==0||nh==0{return Err("Dimensions must be greater than zero".into())}im=im.resize_exact(nw,nh,FilterType::Lanczos3);}
 let fmt=if request.convert_format{request.format.to_lowercase()}else{original_fmt.to_string()};if !["jpg","png","webp","bmp","gif","ico","tiff"].contains(&fmt.as_str()){return Err("Unsupported output format".into())}let target=if request.compress{request.target_bytes.filter(|n|*n>0)}else{None};let mut quality=if request.compress{request.quality.clamp(10,100)}else{100};let mut data=encode(&im,&fmt,quality,request.compress)?;
 let target_met=if let Some(limit)=target {while data.len() as u64>limit&&quality>10{quality=quality.saturating_sub(5).max(10);data=encode(&im,&fmt,quality,true)?;}Some(data.len() as u64<=limit)}else{None};
 let dir=request.output_dir.map(PathBuf::from).unwrap_or_else(||src.parent().unwrap_or(Path::new(".")).to_path_buf());fs::create_dir_all(&dir).map_err(|e|format!("Could not create output folder: {e}"))?;
 let stem=src.file_stem().and_then(|s|s.to_str()).unwrap_or("image");let ext=fmt.as_str();let mut dest=dir.join(format!("{stem}-squeezed.{ext}"));let mut n=2;while dest.exists(){dest=dir.join(format!("{stem}-squeezed-{n}.{ext}"));n+=1}
 fs::write(&dest,&data).map_err(|e|format!("Could not save image: {e}"))?;let message=match(target,target_met){(Some(_),Some(false))=>Some("Target size could not be reached at minimum quality (10%).".into()),_=>None};Ok(ProcessResult{path:dest.to_string_lossy().into_owned(),size:data.len() as u64,target_met,message})
}

#[cfg(windows)]
#[tauri::command] fn open_external_url(url:String)->Result<(),String>{
 let allowed=url==REPOSITORY_URL||url.starts_with("https://github.com/eeliyaa/Squeeze/releases/tag/");
 if !allowed||url.contains(['\r','\n','\0']){return Err("Only the Squeeze repository and its release pages can be opened.".into())}
 use std::os::windows::ffi::OsStrExt;
 let operation="open".encode_utf16().chain(std::iter::once(0)).collect::<Vec<_>>();
 let target=std::ffi::OsStr::new(&url).encode_wide().chain(std::iter::once(0)).collect::<Vec<_>>();
 let result=unsafe{windows_sys::Win32::UI::Shell::ShellExecuteW(std::ptr::null_mut(),operation.as_ptr(),target.as_ptr(),std::ptr::null(),std::ptr::null(),windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL)};
 if result as isize<=32{Err("Windows could not open the link in your default browser.".into())}else{Ok(())}
}
#[cfg(not(windows))]
#[tauri::command] fn open_external_url(_url:String)->Result<(),String>{Err("Opening external links is currently supported on Windows only.".into())}

pub fn run(){tauri::Builder::default().plugin(tauri_plugin_dialog::init()).invoke_handler(tauri::generate_handler![inspect_image,preview_image,process_image,open_external_url]).run(tauri::generate_context!()).expect("error while running Squeeze");}

#[cfg(test)] mod tests {
 use super::*;
 fn fixture()->PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/squeeze-logo.png") }
 fn output_dir()->PathBuf { let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();std::env::temp_dir().join(format!("squeeze-test-{}-{stamp}",std::process::id())) }
 fn operation(path:&Path,dir:&Path,format:&str,convert_format:bool,compress:bool,quality:u8,target:Option<u64>,width:Option<u32>,height:Option<u32>,flip_h:bool,flip_v:bool,rotation:i32)->ProcessRequest { ProcessRequest{path:path.to_string_lossy().into_owned(),output_dir:Some(dir.to_string_lossy().into_owned()),format:format.into(),convert_format,compress,quality,target_bytes:target,width,height,keep_ratio:true,flip_h,flip_v,rotation,crop:None} }
 fn jpeg_fixture(dir:&Path)->PathBuf { let path=dir.join("input.jpg");let rgb=image::open(fixture()).unwrap().to_rgb8();let mut out=Cursor::new(Vec::new());JpegEncoder::new_with_quality(&mut out,100).encode_image(&DynamicImage::ImageRgb8(rgb)).unwrap();fs::write(&path,out.into_inner()).unwrap();path }
 fn request(path:&Path,dir:&Path,format:&str,target:Option<u64>)->ProcessRequest { ProcessRequest{path:path.to_string_lossy().into_owned(),output_dir:Some(dir.to_string_lossy().into_owned()),format:format.into(),convert_format:true,compress:target.is_some(),quality:82,target_bytes:target,width:Some(360),height:None,keep_ratio:true,flip_h:true,flip_v:false,rotation:90,crop:Some(Crop{x:40,y:40,width:700,height:250})} }
 #[test] fn converts_edits_and_never_overwrites_source_or_prior_output(){let src=fixture();let original=fs::read(&src).unwrap();let dir=output_dir();let result=process_image(request(&src,&dir,"jpg",None)).unwrap();let image=image::open(&result.path).unwrap();assert_eq!(image.width(),360);assert!(image.height()>0);assert_eq!(fs::read(&src).unwrap(),original);let second=process_image(request(&src,&dir,"jpg",None)).unwrap();assert_ne!(result.path,second.path);assert!(Path::new(&second.path).exists());fs::remove_dir_all(dir).unwrap();}
 #[test] fn jpeg_webp_and_png_target_reports_measurements(){let src=fixture();let dir=output_dir();let jpeg=process_image(request(&src,&dir,"jpg",Some(10_000_000))).unwrap();assert_eq!(jpeg.target_met,Some(true));assert!(jpeg.size<=10_000_000);let webp=process_image(request(&src,&dir,"webp",Some(10_000_000))).unwrap();assert_eq!(webp.target_met,Some(true));assert!(webp.size<=10_000_000);let too_small=process_image(request(&src,&dir,"webp",Some(1))).unwrap();assert_eq!(too_small.target_met,Some(false));assert!(too_small.message.unwrap().contains("minimum quality"));let png=process_image(request(&src,&dir,"png",Some(10_000_000))).unwrap();assert_eq!(png.target_met,Some(true));assert!(png.size<=10_000_000);let tiny_png=process_image(request(&src,&dir,"png",Some(1))).unwrap();assert_eq!(tiny_png.target_met,Some(false));assert!(tiny_png.message.unwrap().contains("minimum quality"));fs::remove_dir_all(dir).unwrap();}
 #[test] fn png_horizontal_flip_only_stays_png_and_preserves_source(){let src=fixture();let original=fs::read(&src).unwrap();let before=image::open(&src).unwrap();let dir=output_dir();let req=operation(&src,&dir,"jpg",false,false,10,Some(1),None,None,true,false,0);let result=process_image(req).unwrap();assert_eq!(image::guess_format(&fs::read(&result.path).unwrap()).unwrap(),ImageFormat::Png);let before_rgba=before.to_rgba8();let after=image::open(&result.path).unwrap().to_rgba8();assert_eq!(after.dimensions(),before_rgba.dimensions());for (x,y,p) in before_rgba.enumerate_pixels().step_by(5000){assert_eq!(*p,*after.get_pixel(before_rgba.width()-1-x,y));}assert_eq!(fs::read(&src).unwrap(),original);fs::remove_dir_all(dir).unwrap();}
 #[test] fn jpg_rotation_only_stays_jpg_without_compression(){let dir=output_dir();fs::create_dir_all(&dir).unwrap();let src=jpeg_fixture(&dir);let input=image::open(&src).unwrap();let result=process_image(operation(&src,&dir,"webp",false,false,10,Some(1),None,None,false,false,90)).unwrap();let low_quality=process_image(operation(&src,&dir,"jpg",false,true,10,None,None,None,false,false,0)).unwrap();assert!(result.path.ends_with(".jpg"));assert_eq!(image::guess_format(&fs::read(&result.path).unwrap()).unwrap(),ImageFormat::Jpeg);let output=image::open(&result.path).unwrap();assert_eq!(output.dimensions(),(input.height(),input.width()));assert!(result.size>low_quality.size,"uncompressed rotation should not use the low compression quality setting");fs::remove_dir_all(dir).unwrap();}
 #[test] fn png_to_webp_conversion_only_is_lossless(){let src=fixture();let before=image::open(&src).unwrap().to_rgba8();let dir=output_dir();let result=process_image(operation(&src,&dir,"webp",true,false,10,None,None,None,false,false,0)).unwrap();assert_eq!(image::guess_format(&fs::read(&result.path).unwrap()).unwrap(),ImageFormat::WebP);assert_eq!(image::open(&result.path).unwrap().to_rgba8(),before);fs::remove_dir_all(dir).unwrap();}
 #[test] fn jpg_compression_only_uses_quality_and_keeps_jpg(){let dir=output_dir();fs::create_dir_all(&dir).unwrap();let src=jpeg_fixture(&dir);let input_size=fs::metadata(&src).unwrap().len();let result=process_image(operation(&src,&dir,"png",false,true,35,None,None,None,false,false,0)).unwrap();assert!(result.path.ends_with(".jpg"));assert_eq!(image::guess_format(&fs::read(&result.path).unwrap()).unwrap(),ImageFormat::Jpeg);assert!(result.size<input_size,"compressed size {} should be below {}",result.size,input_size);fs::remove_dir_all(dir).unwrap();}
 #[test] fn resize_only_and_mixed_batch_preserve_each_source_format(){let dir=output_dir();fs::create_dir_all(&dir).unwrap();let png=fixture();let jpg=jpeg_fixture(&dir);let resized=process_image(operation(&png,&dir,"webp",false,false,10,None,Some(400),None,false,false,0)).unwrap();assert!(resized.path.ends_with(".png"));assert_eq!(image::open(&resized.path).unwrap().dimensions(),(400,110));for src in [&png,&jpg]{let result=process_image(operation(src,&dir,"webp",false,false,10,None,None,None,false,true,0)).unwrap();let expected=if src==&png{ImageFormat::Png}else{ImageFormat::Jpeg};assert_eq!(image::guess_format(&fs::read(&result.path).unwrap()).unwrap(),expected);}fs::remove_dir_all(dir).unwrap();}
 #[test] fn bmp_gif_ico_and_tiff_encoders_round_trip_real_image(){let src=fixture();let original=image::open(&src).unwrap();let formats=[("bmp",ImageFormat::Bmp),("gif",ImageFormat::Gif),("ico",ImageFormat::Ico),("tiff",ImageFormat::Tiff)];for (format,expected) in formats{let dir=output_dir();let result=process_image(operation(&src,&dir,format,true,false,100,None,None,None,false,false,0)).unwrap();let bytes=fs::read(&result.path).unwrap();assert_eq!(image::guess_format(&bytes).unwrap(),expected,"{format} signature");let decoded=image::load_from_memory(&bytes).unwrap();if format=="ico"{assert!(decoded.width()<=256&&decoded.height()<=256,"ICO dimensions must fit the format");}else{assert_eq!(decoded.dimensions(),original.dimensions(),"{format} dimensions");}if format=="gif"{assert!(!is_animated_gif(&bytes,expected).unwrap(),"static GIF conversion must remain a single frame");}fs::remove_dir_all(dir).unwrap();}}
 #[test] fn animated_gif_is_detected_and_refused_without_flattening(){use image::{codecs::gif::GifEncoder,Frame,Rgba,RgbaImage};let dir=output_dir();fs::create_dir_all(&dir).unwrap();let src=dir.join("animated.gif");let first=Frame::new(RgbaImage::from_pixel(3,2,Rgba([255,0,0,255])));let second=Frame::new(RgbaImage::from_pixel(3,2,Rgba([0,0,255,255])));let mut output=Cursor::new(Vec::new());let mut encoder=GifEncoder::new(&mut output);encoder.encode_frames([first,second]).unwrap();drop(encoder);fs::write(&src,output.into_inner()).unwrap();let bytes=fs::read(&src).unwrap();assert!(is_animated_gif(&bytes,ImageFormat::Gif).unwrap());let error=process_image(operation(&src,&dir,"png",true,false,100,None,None,None,true,false,0)).err().unwrap();assert!(error.contains("Animated GIFs"));assert_eq!(fs::read(&src).unwrap(),bytes);fs::remove_dir_all(dir).unwrap();}
 #[test] fn rejects_no_operation_without_creating_output(){let src=fixture();let dir=output_dir();assert!(process_image(operation(&src,&dir,"jpg",false,false,82,None,None,None,false,false,0)).err().unwrap().contains("operation"));assert!(!dir.exists());}
}

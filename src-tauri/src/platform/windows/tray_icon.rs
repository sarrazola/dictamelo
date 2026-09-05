//! Ícono de la bandeja adaptado a Windows: la bandeja espera íconos cuadrados y no tiñe las
//! «plantillas» como macOS, así que el trazo negro del ícono de reposo se vuelve blanco cuando
//! la barra de tareas es oscura. Los íconos de color (grabando, transcribiendo…) se dejan igual.

use super::registry_dword;
use tauri::image::Image;
use windows::Win32::System::Registry::HKEY_CURRENT_USER;

const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

/// `true` si la barra de tareas usa el tema oscuro (valor ausente = oscuro, el predeterminado
/// de Windows 10).
fn taskbar_is_dark() -> bool {
    registry_dword(HKEY_CURRENT_USER, PERSONALIZE, "SystemUsesLightTheme").unwrap_or(0) == 0
}

pub fn tray_icon(bytes: &'static [u8]) -> Image<'static> {
    let image = Image::from_bytes(bytes).expect("los íconos PNG embebidos son válidos");
    let (width, height) = (image.width() as usize, image.height() as usize);
    let side = width.max(height);
    let (offset_x, offset_y) = ((side - width) / 2, (side - height) / 2);
    let invert = taskbar_is_dark();
    let source = image.rgba();
    let mut rgba = vec![0u8; side * side * 4];
    for y in 0..height {
        for x in 0..width {
            let from = (y * width + x) * 4;
            let to = ((y + offset_y) * side + (x + offset_x)) * 4;
            let [r, g, b, a] = [source[from], source[from + 1], source[from + 2], source[from + 3]];
            let (r, g, b) = if invert && r < 40 && g < 40 && b < 40 { (255, 255, 255) } else { (r, g, b) };
            rgba[to..to + 4].copy_from_slice(&[r, g, b, a]);
        }
    }
    Image::new_owned(rgba, side as u32, side as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icons_become_square_and_keep_alpha() {
        for bytes in [
            include_bytes!("../../../icons/tray/idle.png").as_slice(),
            include_bytes!("../../../icons/tray/recording.png").as_slice(),
        ] {
            let original = Image::from_bytes(bytes).unwrap();
            let icon = tray_icon(bytes);
            assert_eq!(icon.width(), icon.height());
            assert!(icon.width() >= original.width() && icon.height() >= original.height());
            let opaque = |img: &Image| img.rgba().chunks_exact(4).filter(|p| p[3] > 0).count();
            assert_eq!(opaque(&icon), opaque(&original));
        }
    }
}

//! Roboto local: o mesmo TTF serve ao GTK e à página inicial.
use gtk4::glib::prelude::*;
use std::{
    fs::File,
    io::Write,
    os::fd::{AsRawFd, FromRawFd},
    sync::OnceLock,
};

const ROBOTO: &[u8] = include_bytes!("../assets/fonts/Roboto-Latin-Variable.ttf");

#[link(name = "fontconfig")]
unsafe extern "C" {
    fn FcConfigAppFontAddFile(config: *mut libc::c_void, file: *const u8) -> libc::c_int;
}

#[link(name = "pangoft2-1.0")]
unsafe extern "C" {
    fn pango_fc_font_map_get_config(font_map: *mut libc::c_void) -> *mut libc::c_void;
    fn pango_fc_font_map_set_config(font_map: *mut libc::c_void, config: *mut libc::c_void);
    fn pango_fc_font_map_config_changed(font_map: *mut libc::c_void);
}

/// Registra a fonte apenas neste processo, sem instalar arquivos no sistema.
/// O descritor permanece aberto porque FreeType pode reabrir a fonte por caminho.
pub fn register_native(font_map: &gtk4::pango::FontMap) -> Result<(), &'static str> {
    let fc_type = gtk4::glib::Type::from_name("PangoFcFontMap")
        .ok_or("O backend Fontconfig não está disponível.")?;
    if !font_map.type_().is_a(fc_type) {
        return Err("O mapa de fontes não usa Fontconfig.");
    }
    static FONT: OnceLock<Result<File, &'static str>> = OnceLock::new();
    let file = FONT
        .get_or_init(|| {
            // SAFETY: nome terminado em NUL e flags válidas da API Linux.
            let descriptor =
                unsafe { libc::memfd_create(c"kairo-roboto.ttf".as_ptr(), libc::MFD_CLOEXEC) };
            if descriptor < 0 {
                return Err("Não foi possível criar a fonte local em memória.");
            }
            // SAFETY: memfd_create retornou um descritor novo, transferido para File.
            let mut file = unsafe { File::from_raw_fd(descriptor) };
            file.write_all(ROBOTO)
                .map_err(|_| "Não foi possível carregar a fonte local.")?;
            Ok(file)
        })
        .as_ref()
        .map_err(|error| *error)?;
    let path = std::ffi::CString::new(format!("/proc/self/fd/{}", file.as_raw_fd()))
        .expect("Descritor numérico sem NUL");
    // SAFETY: o tipo foi validado como PangoFcFontMap. A configuração pertence
    // ao mapa e não é liberada aqui; NULL seleciona a configuração padrão.
    let config = unsafe { pango_fc_font_map_get_config(font_map.as_ptr().cast()) };
    // SAFETY: caminho válido durante a chamada; File fica vivo enquanto o app roda.
    let loaded = unsafe { FcConfigAppFontAddFile(config, path.as_ptr().cast()) };
    if loaded == 0 {
        return Err("O Fontconfig não conseguiu registrar a Roboto local.");
    }

    // SAFETY: o mapa continua vivo e seu tipo Fc foi validado acima. Esse aviso
    // invalida os caches de resolução de fontes após o registro no Fontconfig.
    unsafe {
        pango_fc_font_map_set_config(font_map.as_ptr().cast(), config);
        pango_fc_font_map_config_changed(font_map.as_ptr().cast());
    }
    Ok(())
}

pub fn roboto_data_uri() -> String {
    format!("data:font/ttf;base64,{}", gtk4::glib::base64_encode(ROBOTO))
}

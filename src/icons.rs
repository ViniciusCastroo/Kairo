//! Subconjunto SVG do Lucide embarcado como ícones simbólicos do GTK.
pub const BACK: &str = "kairo-arrow-left-symbolic";
pub const FORWARD: &str = "kairo-arrow-right-symbolic";
pub const RELOAD: &str = "kairo-rotate-cw-symbolic";
pub const CLOSE: &str = "kairo-x-symbolic";
pub const PLUS: &str = "kairo-plus-symbolic";
pub const SEARCH: &str = "kairo-search-symbolic";
pub const STAR: &str = "kairo-star-symbolic";
pub const MENU: &str = "kairo-menu-symbolic";
pub const FOLDER: &str = "kairo-folder-open-symbolic";
pub const EDIT: &str = "kairo-pencil-symbolic";
pub const UP: &str = "kairo-arrow-up-symbolic";
pub const DOWN: &str = "kairo-arrow-down-symbolic";
pub const TRASH: &str = "kairo-trash-symbolic";

pub const ALL: [&str; 13] = [
    BACK, FORWARD, RELOAD, CLOSE, PLUS, SEARCH, STAR, MENU, FOLDER, EDIT, UP, DOWN, TRASH,
];

pub fn search_svg() -> String {
    decorative(include_str!(
        "../assets/icons/scalable/actions/kairo-search-symbolic.svg"
    ))
}

pub fn forward_svg() -> String {
    decorative(include_str!(
        "../assets/icons/scalable/actions/kairo-arrow-right-symbolic.svg"
    ))
}

pub fn edit_svg() -> String {
    decorative(include_str!(
        "../assets/icons/scalable/actions/kairo-pencil-symbolic.svg"
    ))
}

fn decorative(svg: &str) -> String {
    svg.replacen("<svg", "<svg aria-hidden=\"true\" focusable=\"false\"", 1)
}

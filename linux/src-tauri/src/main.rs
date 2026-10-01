// Coucou: Mochi is the whole UI, so there is nothing to show in a terminal.

fn main() {
    // WebKitGTK's DMA-BUF renderer kills the whole app with "Error 71 (Protocol
    // error) dispatching to Wayland display" on a number of Wayland setups
    // (seen on Arch/CachyOS). The island is a small 2D canvas, so the plain
    // renderer costs nothing visible. Set it yourself (to 0) to opt back in.
    // This runs before any thread exists, which is what makes set_var sound.
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    coucou_lib::run()
}

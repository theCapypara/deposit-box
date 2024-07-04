use comrak::{markdown_to_html, ExtensionOptions, Options, ParseOptions, RenderOptions};
use std::sync::OnceLock;

pub fn markdown(input: &str) -> String {
    static DEFAULT_OPTIONS: OnceLock<Options> = OnceLock::new();

    let options = DEFAULT_OPTIONS.get_or_init(|| {
        let mut extension = ExtensionOptions::default();
        extension.strikethrough = true;
        extension.tagfilter = true;
        extension.table = true;
        extension.autolink = true;
        let parse = ParseOptions::default();
        let mut render = RenderOptions::default();
        render.unsafe_ = false;
        render.escape = true;

        Options {
            extension,
            parse,
            render,
        }
    });

    markdown_to_html(input, options)
}

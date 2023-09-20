use comrak::{
    markdown_to_html, ComrakExtensionOptions, ComrakOptions, ComrakParseOptions,
    ComrakRenderOptions, ListStyleType,
};

pub fn markdown(input: &str) -> String {
    const DEFAULT_OPTIONS: ComrakOptions = ComrakOptions {
        extension: ComrakExtensionOptions {
            strikethrough: true,
            tagfilter: true,
            table: true,
            autolink: true,
            // default:
            tasklist: false,
            superscript: false,
            header_ids: None,
            footnotes: false,
            description_lists: false,
            front_matter_delimiter: None,
        },
        parse: ComrakParseOptions {
            // default:
            smart: false,
            default_info_string: None,
            relaxed_tasklist_matching: false,
        },
        render: ComrakRenderOptions {
            unsafe_: false,
            escape: true,
            // default:
            list_style: ListStyleType::Dash,
            hardbreaks: false,
            github_pre_lang: false,
            full_info_string: false,
            width: 0,
            sourcepos: false,
        },
    };

    markdown_to_html(input, &DEFAULT_OPTIONS)
}

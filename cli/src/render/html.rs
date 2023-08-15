use handlebars::Handlebars;
use log::debug;
use serde_json::json;

use crate::theme;

pub struct HtmlRenderer {
    // html renderer
    pub handlebars: Handlebars<'static>,
}

impl HtmlRenderer {
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();
        // todo
        let theme = theme::Theme::new(std::path::Path::new("themes/mdbook"));

        debug!("Register the index handlebars template");
        handlebars
            .register_template_string("index", String::from_utf8(theme.index.clone()).unwrap())
            .unwrap();
        handlebars
            .register_template_string(
                "typst_load_trampoline",
                String::from_utf8(theme.typst_load_trampoline.clone()).unwrap(),
            )
            .unwrap();

        // todo: helpers
        // debug!("Register handlebars helpers");
        handlebars.register_helper(
            "toc",
            Box::new(RenderToc {
                no_section_label: false,
            }),
        );

        Self { handlebars }
    }

    pub fn render_index(&mut self, mut data: DataDict, path: &str) -> String {
        // inject path (for current document)
        data.insert("path".to_owned(), json!(path));

        data.insert("fold_enable".to_owned(), json!(false));
        data.insert("fold_level".to_owned(), json!(0u64));
        data.insert("default_theme".to_owned(), json!("light"));
        data.insert("book_title".to_owned(), data["title"].clone());
        if let Some(repo) = data.get("repository") {
            data.insert("git_repository_url".to_owned(), repo.clone());
            data.insert("git_repository_icon".to_owned(), json!("fa-github"));
        }
        // git_repository_edit_url
        if let Some(repo_edit) = data.get("repository-edit") {
            data.insert("git_repository_edit_url".to_owned(), repo_edit.clone());
        } else if let Some(repo) = data.get("repository") {
            data.insert(
                "git_repository_edit_url".to_owned(),
                json!(format!("{}/edit/master/", repo.as_str().unwrap())),
            );
        }
        // data.insert("git_repository_url".to_owned(), data["repository"].clone());

        // inject path_to_root
        // todo: path_to_root
        data.insert("path_to_root".to_owned(), json!("/"));

        self.handlebars.render("index", &data).unwrap()
    }
}

use std::{cmp::Ordering, collections::BTreeMap, path::Path};

pub(crate) fn bracket_escape(mut s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    let needs_escape: &[char] = &['<', '>'];
    while let Some(next) = s.find(needs_escape) {
        escaped.push_str(&s[..next]);
        match s.as_bytes()[next] {
            b'<' => escaped.push_str("&lt;"),
            b'>' => escaped.push_str("&gt;"),
            _ => unreachable!(),
        }
        s = &s[next + 1..];
    }
    escaped.push_str(s);
    escaped
}

use handlebars::{Context, Helper, HelperDef, Output, RenderContext, RenderError};

use super::DataDict;

// Handlebars helper to construct TOC
#[derive(Clone, Copy)]
pub struct RenderToc {
    pub no_section_label: bool,
}

impl HelperDef for RenderToc {
    fn call<'reg: 'rc, 'rc>(
        &self,
        _h: &Helper<'reg, 'rc>,
        _r: &'reg Handlebars<'_>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> Result<(), RenderError> {
        println!("RC = {:?}", rc);

        // get value from context data
        // rc.get_path() is current json parent path, you should always use it like this
        // param is the key of value you want to display
        let chapters = rc.evaluate(ctx, "@root/chapters").and_then(|c| {
            println!("scopedjson = {:?}", c);
            serde_json::value::from_value::<Vec<BTreeMap<String, String>>>(c.as_json().clone())
                .map_err(|_| RenderError::new("Could not decode the JSON data"))
        })?;

        let path_to_root = rc
            .evaluate(ctx, "@root/path_to_root")?
            .as_json()
            .as_str()
            .ok_or_else(|| RenderError::new("Type error for `path_to_root`, string expected"))?
            .replace('\"', "");

        let current_path = rc
            .evaluate(ctx, "@root/path")?
            .as_json()
            .as_str()
            .ok_or_else(|| RenderError::new("Type error for `path`, string expected"))?
            .replace('\"', "");

        let current_section = rc
            .evaluate(ctx, "@root/section")?
            .as_json()
            .as_str()
            .map(str::to_owned)
            .unwrap_or_default();

        let fold_enable = rc
            .evaluate(ctx, "@root/fold_enable")?
            .as_json()
            .as_bool()
            .ok_or_else(|| RenderError::new("Type error for `fold_enable`, bool expected"))?;

        let fold_level = rc
            .evaluate(ctx, "@root/fold_level")?
            .as_json()
            .as_u64()
            .ok_or_else(|| RenderError::new("Type error for `fold_level`, u64 expected"))?;

        out.write("<ol class=\"chapter\">")?;

        let mut current_level = 1;
        // The "index" page, which has this attribute set, is supposed to alias the
        // first chapter in the book, i.e. the first link. There seems to be no
        // easy way to determine which chapter the "index" is aliasing from
        // within the renderer, so this is used instead to force the first link
        // to be active. See further below.
        let mut is_first_chapter = ctx.data().get("is_index").is_some();

        for item in chapters {
            // Spacer
            if item.get("spacer").is_some() {
                out.write("<li class=\"spacer\"></li>")?;
                continue;
            }

            let (section, level) = if let Some(s) = item.get("section") {
                (s.as_str(), s.matches('.').count())
            } else {
                ("", 1)
            };

            let is_expanded =
                if !fold_enable || (!section.is_empty() && current_section.starts_with(section)) {
                    // Expand if folding is disabled, or if the section is an
                    // ancestor or the current section itself.
                    true
                } else {
                    // Levels that are larger than this would be folded.
                    level - 1 < fold_level as usize
                };

            match level.cmp(&current_level) {
                Ordering::Greater => {
                    while level > current_level {
                        out.write("<li>")?;
                        out.write("<ol class=\"section\">")?;
                        current_level += 1;
                    }
                    write_li_open_tag(out, is_expanded, false)?;
                }
                Ordering::Less => {
                    while level < current_level {
                        out.write("</ol>")?;
                        out.write("</li>")?;
                        current_level -= 1;
                    }
                    write_li_open_tag(out, is_expanded, false)?;
                }
                Ordering::Equal => {
                    write_li_open_tag(out, is_expanded, item.get("section").is_none())?;
                }
            }

            // Part title
            if let Some(title) = item.get("part") {
                out.write("<li class=\"part-title\">")?;
                out.write(&bracket_escape(title))?;
                out.write("</li>")?;
                continue;
            }

            // Link
            let path_exists: bool;
            match item.get("path") {
                Some(path) if !path.is_empty() => {
                    out.write("<a href=\"")?;
                    let tmp = Path::new(&path_to_root)
                        .join(path)
                        .with_extension("html")
                        .to_str()
                        .unwrap()
                        // Hack for windows who tends to use `\` as separator instead of `/`
                        .replace('\\', "/");

                    // Add link
                    // out.write(&path_to_root(&current_path))?;
                    out.write(&tmp)?;
                    out.write("\"")?;

                    println!("compare path = {path:?}, current_path = {current_path:?}");

                    if path == &current_path || is_first_chapter {
                        is_first_chapter = false;
                        out.write(" class=\"active\"")?;
                    }

                    out.write(">")?;
                    path_exists = true;
                }
                _ => {
                    out.write("<div>")?;
                    path_exists = false;
                }
            }

            if !self.no_section_label {
                // Section does not necessarily exist
                if let Some(section) = item.get("section") {
                    out.write("<strong aria-hidden=\"true\">")?;
                    out.write(section)?;
                    out.write("</strong> ")?;
                }
            }

            if let Some(name) = item.get("name") {
                out.write(&bracket_escape(name))?
            }

            if path_exists {
                out.write("</a>")?;
            } else {
                out.write("</div>")?;
            }

            // Render expand/collapse toggle
            if let Some(flag) = item.get("has_sub_items") {
                let has_sub_items = flag.parse::<bool>().unwrap_or_default();
                if fold_enable && has_sub_items {
                    out.write("<a class=\"toggle\"><div>❱</div></a>")?;
                }
            }
            out.write("</li>")?;
        }
        while current_level > 1 {
            out.write("</ol>")?;
            out.write("</li>")?;
            current_level -= 1;
        }

        out.write("</ol>")?;
        Ok(())
    }
}

fn write_li_open_tag(
    out: &mut dyn Output,
    is_expanded: bool,
    is_affix: bool,
) -> Result<(), std::io::Error> {
    let mut li = String::from("<li class=\"chapter-item ");
    if is_expanded {
        li.push_str("expanded ");
    }
    if is_affix {
        li.push_str("affix ");
    }
    li.push_str("\">");
    out.write(&li)
}

impl Default for HtmlRenderer {
    fn default() -> Self {
        Self::new()
    }
}

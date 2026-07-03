use pulldown_cmark::CowStr;

/// Converts a markdown string to a groff/troff string.
///
/// * `title` is the name of the program. It's typically all-uppercase.
///
/// * `section` is the numeric section of man pages. Usually `1`.
///
/// The conversion is very rough. HTML fragments are merely stripped from tags.
/// GitHub tables extension is not supported.
#[must_use]
pub fn convert(markdown_markup: &str, title: &str, section: u8) -> String {
    use pulldown_cmark::Event::*;
    use pulldown_cmark::Tag::*;
    use pulldown_cmark::{Options, Parser, TagEnd, LinkType, BlockQuoteKind};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_GFM);
    options.insert(Options::ENABLE_DEFINITION_LIST);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(markdown_markup, options);

    let mut out = Rough {
        out: String::new(),
        in_quotes: false,
        unclosed_table_cell: false,
        bold_level: 0,
        italic_level: 0,
    };
    out.title(title, section);

    let mut links = Links {
        regular: Vec::new(),
        deferred: Vec::new(),
    };
    let mut min_header_level = 999;
    let mut last_header_level = 999;
    let mut link_ref_num = 0;
    let mut list_item_num = None;
    let mut in_list = false;
    let mut in_footnote = false;
    let mut first_para_in_list = false;

    let mut html_state = TagStrip {
        state: HtmlState::Text,
        skip_content: false,
    };

    let mut url_stack = Vec::new();
    for event in parser {
        match event {
            Rule => {
                html_state.reset();
                links.flush(&mut out, false);
                out.centered("----")
            },
            Html(markup) | DisplayMath(markup) => {
                out.ensure_line_start();
                out.text(&html_state.strip_tags(&markup));
            },
            InlineHtml(markup) | InlineMath(markup) => {
                html_state.reset();
                out.text(&html_state.strip_tags(&markup));
            },
            TaskListMarker(checked) => out.text(if checked {"[x]"} else {"[ ]"}),
            Start(Heading { level, .. }) => {
                let n = level as u32;
                links.flush(&mut out, n < last_header_level);
                last_header_level = n;

                if n < min_header_level {
                    min_header_level = n;
                }
                out.section_title_start(n + 1 - min_header_level);
            },
            End(TagEnd::Heading(n)) => {
                out.section_title_end((n as u32) + 1 - min_header_level);

                links.flush(&mut out, false);
            },

            Start(tag @ (Link { .. } | Image { .. })) => {
                let is_image = matches!(tag, Image { .. });
                let (Link { dest_url, link_type, mut title, mut id } | Image { dest_url, link_type, mut title, mut id }) = tag else {
                    break;
                };
                let to_stack = match link_type {
                    LinkType::Autolink |
                    LinkType::Email => None,
                    _ => {
                        let shortcut_reference = matches!(link_type, LinkType::Shortcut | LinkType::ShortcutUnknown);
                        if shortcut_reference {
                            out.text("[");
                        }
                        let defer = is_image || shortcut_reference || matches!(link_type, LinkType::Reference | LinkType::ReferenceUnknown | LinkType::Collapsed | LinkType::CollapsedUnknown);
                        if title == dest_url {
                            title = "".into();
                        }
                        if !shortcut_reference && id.len() > 5 {
                            id = "".into();
                        }
                        Some((dest_url, title, id, is_image, defer, shortcut_reference))
                    }
                };
                url_stack.push(to_stack);
            },
            End(TagEnd::Link | TagEnd::Image) => {
                if let Some((url, title, mut id, is_image, defer, shortcut_reference)) = url_stack.pop().flatten() {
                    let mut matches_existing = false;
                    for (old_url, old_title, old_id) in links.regular.iter().chain(&links.deferred) {
                        if url == *old_url && (title.is_empty() || title == *old_title) && (id.is_empty() || id == *old_id) {
                            matches_existing = true;
                            if id.is_empty() {
                                id = old_id.clone();
                            }
                        }
                    }
                    let id = if !id.is_empty() { id } else {
                        link_ref_num += 1;
                        format!("{}{link_ref_num}", if is_image { "img" } else { "" }).into()
                    };
                    if !shortcut_reference {
                        out.text(&format!("[{id}]"));
                    } else {
                        out.text("]");
                    }

                    if !matches_existing {
                        if defer {
                            &mut links.deferred
                        } else {
                            &mut links.regular
                        }.push((url, title, id));
                    }
                }
            },
            Start(CodeBlock(_)) => out.pre_start(),
            End(TagEnd::CodeBlock) => out.pre_end(),

            Start(List(num)) => {
                list_item_num = num;
                out.indent();
            },
            End(TagEnd::List(_)) => {
                out.outdent();
                links.flush(&mut out, false);
                list_item_num = None;
            },

            Start(Item) => {
                in_list = true;
                first_para_in_list = true;
                out.list_item_start(list_item_num);
                if let Some(n) = &mut list_item_num {
                    *n += 1;
                }
            },
            End(TagEnd::Item) => {
                in_list = false;
                first_para_in_list = false;
                out.list_item_end();
            },

            Start(BlockQuote(kind)) => {
                out.blockquote_start();
                if let Some(kind) = kind {
                    out.italic_start();
                    out.text(match kind {
                        BlockQuoteKind::Note => "Note",
                        BlockQuoteKind::Tip => "Tip",
                        BlockQuoteKind::Important => "Important",
                        BlockQuoteKind::Warning => "Warning",
                        BlockQuoteKind::Caution => "Caution",
                    });
                    out.italic_end();
                    out.line_break();
                }
            },
            End(TagEnd::BlockQuote(_)) => {
                links.flush(&mut out, false);
                out.blockquote_end();
            },

            Start(Paragraph) => {
                html_state.reset();
                if in_list {
                    if first_para_in_list {
                        first_para_in_list = false;
                    } else {
                        out.empty_line();
                    }
                } else if !in_footnote {
                    out.paragraph_start();
                }
            },
            End(TagEnd::Paragraph) => {
                out.paragraph_end();
            },

            Start(Emphasis) => out.italic_start(),
            End(TagEnd::Emphasis) => out.italic_end(),

            Start(Strikethrough) => {
                out.text("~"); out.italic_start();
            },
            End(TagEnd::Strikethrough) => {
                out.italic_end(); out.text("~");
            },

            Start(Strong) => out.bold_start(),
            End(TagEnd::Strong) => out.bold_end(),

            Start(Superscript) => { out.text("^"); },
            End(TagEnd::Superscript) => {},
            Start(Subscript) => { out.text("_"); },
            End(TagEnd::Subscript) => {},

            HardBreak => out.line_break(),
            SoftBreak => out.ensure_line_start(),
            Code(text) => out.code(&text),
            Text(text) => out.text(&text),
            FootnoteReference(s) => {
                out.text(&format!("[^{s}]"));
            }
            Start(FootnoteDefinition(s)) => {
                in_footnote = true;
                out.empty_line();
                out.text(&format!("[^{s}]: "));
            },
            End(TagEnd::FootnoteDefinition) => {
                in_footnote = false;
                links.flush(&mut out, true);
                out.ensure_line_start();
            },

            Start(HtmlBlock) => out.paragraph_start(),
            End(TagEnd::HtmlBlock) => out.paragraph_end(),

            Start(MetadataBlock(_)) |
            End(TagEnd::MetadataBlock(_)) => out.ensure_line_start(),

            Start(Table(_)) => {
                html_state.reset();
                out.table_start();
            },
            End(TagEnd::Table) => out.table_end(),
            Start(TableRow) | Start(TableHead) => out.table_row_start(),
            End(TagEnd::TableRow) | End(TagEnd::TableHead) => out.table_row_end(),
            Start(TableCell) => out.table_cell_start(),
            End(TagEnd::TableCell) => out.table_cell_end(),

            Start(DefinitionList) => {
                html_state.reset();
                out.ensure_line_start();
            },
            Start(DefinitionListDefinition) => {
                out.indent();
            }
            End(TagEnd::DefinitionListDefinition) => {
                out.outdent();
            }
            End(TagEnd::DefinitionList) => {
                links.flush(&mut out, false);
            },
            Start(DefinitionListTitle) => out.bold_start(),
            End(TagEnd::DefinitionListTitle) => {
                out.bold_end();
                out.ensure_line_start();
            },
        }
    }

    links.flush(&mut out, true);

    out.out
}

struct Links<'a> {
    regular: Vec<(CowStr<'a>, CowStr<'a>, CowStr<'a>)>,
    deferred: Vec<(CowStr<'a>, CowStr<'a>, CowStr<'a>)>,
}

impl Links<'_> {
    pub fn flush(&mut self, out: &mut Rough, flush_all: bool) {
        let num_links = self.regular.len() + self.deferred.len();
        if num_links < 10 && self.regular.is_empty() && (!flush_all || self.deferred.is_empty()) {
            return;
        }

        out.empty_line();
        for (url, title, id) in self.deferred.drain(..).chain(self.regular.drain(..)) {
            out.text(&format!("[{id}]: {url}"));
            if !title.is_empty() {
                out.text(&format!(" {title}"))
            }
            out.line_break();
        }
    }
}

struct Rough {
    out: String,
    in_quotes: bool,
    unclosed_table_cell: bool,
    bold_level: u8,
    italic_level: u8,
}

impl Rough {
    pub fn title(&mut self, title: &str, man_section: u8) {
        self.ensure_line_start();
        self.out.push_str(".TH \"");
        self.in_quotes = true;
        self.text(title);
        self.in_quotes = false;
        self.text(&format!("\" {man_section}"));
        self.out.push('\n');
    }

    pub fn section_title_start(&mut self, level: u32) {
        self.ensure_line_start();
        self.in_quotes = true;
        // extra line needed too, otherwise headers get wrapped into prev paragraph?
        self.out.push('\n');
        match level {
            1 => self.out.push_str(".SH \""),
            2 => self.out.push_str(".SS \""),
            _ => self.out.push_str(".SB \""),
        }
    }

    pub fn section_title_end(&mut self, _level: u32) {
        self.in_quotes = false;
        self.out.push_str("\"\n");
    }

    pub fn table_start(&mut self) {
        self.paragraph_start();
        // self.out.push_str(".TS\n");
    }

    pub fn table_end(&mut self) {
        self.paragraph_end();
        // self.out.push_str(".TE\n");
    }

    pub fn table_row_start(&mut self) {
        self.ensure_line_start();
    }

    pub fn table_row_end(&mut self) {
        if self.unclosed_table_cell {
            self.unclosed_table_cell = false;
            self.text("\t|");
        }
        self.line_break()
    }

    pub fn table_cell_start(&mut self) {
        self.text(if self.unclosed_table_cell { "\t| " } else { "| " });
    }

    pub fn table_cell_end(&mut self) {
        self.unclosed_table_cell = true;
    }

    pub fn paragraph_start(&mut self) {
        self.ensure_line_start();
        self.out.push_str(".PP\n");
    }

    pub fn paragraph_end(&mut self) {
        debug_assert_eq!(0, self.bold_level);
        debug_assert_eq!(0, self.italic_level);
        self.out.push('\n');
    }

    pub fn blockquote_start(&mut self) {
        self.indent();
        // self.ensure_line_start();
        // self.out.push_str(".QS\n");
    }

    pub fn blockquote_end(&mut self) {
        self.outdent();
        // self.ensure_line_start();
        // self.out.push_str(".QE\n");
    }

    pub fn list_item_start(&mut self, n: Option<u64>) {
        self.ensure_line_start();
        self.out.push_str(".Bl\n");
        if let Some(n) = n {
            self.out.push_str(&format!(".IP {n}. 4\n"));
        } else {
            self.out.push_str(".IP \\(bu 4\n");
        }
    }

    pub fn list_item_end(&mut self) {
        self.ensure_line_start();
        self.out.push_str(".El\n");
    }

    pub fn ensure_line_start(&mut self) {
        if self.out.is_empty() || self.out.ends_with('\n') {
            return;
        }
        self.out.push('\n');
    }

    pub fn text(&mut self, text: &str) {
        let text = deunicode::deunicode_with_tofu_cow(text, "[?]");
        let text = if self.in_quotes {
            text.replace('"', "\"\"")
        } else {
            text.replace('-', "\\-").replace('.', "\\.")
        };
        self.out.push_str(&text);
    }

    pub fn code(&mut self, text: &str) {
        self.out.push_str("`\\f[CR]");
        self.text(text);
        self.out.push_str("\\fP`");
    }

    pub fn pre_start(&mut self) {
        self.indent();
        self.ensure_line_start();
        self.out.push_str(".PP\n");
        self.out.push_str(".nf\n");
    }
    pub fn pre_end(&mut self) {
        self.ensure_line_start();
        self.out.push_str(".fi\n");
        self.outdent();
    }

    pub fn line_break(&mut self) {
        self.ensure_line_start();
        self.out.push_str(".nf\n.fi\n");
    }

    pub fn empty_line(&mut self) {
        self.ensure_line_start();
        self.out.push_str(".sp\n");
    }

    pub fn italic_start(&mut self) {
        self.italic_level += 1;
        self.out.push_str("\\fI");
    }
    pub fn italic_end(&mut self) {
        self.italic_level -= 1;
        self.out.push_str("\\fP");
    }

    pub fn bold_start(&mut self) {
        self.bold_level += 1;
        self.out.push_str("\\fB");
    }
    pub fn bold_end(&mut self) {
        self.bold_level -= 1;
        self.out.push_str("\\fP");
    }

    pub fn indent(&mut self) {
        self.ensure_line_start();
        self.out.push_str(".RS\n");
    }
    pub fn outdent(&mut self) {
        self.ensure_line_start();
        self.out.push_str(".RE\n");
    }
    pub fn centered(&mut self, text: &str) {
        self.ensure_line_start();
        self.out.push_str(".ce 1000\n");
        self.text(text);
        self.ensure_line_start();
        self.out.push_str(".ce 0\n");
    }
}

struct TagStrip {
    state: HtmlState,
    skip_content: bool,
}

enum HtmlState {
    Text,
    Lt,
    Name,
    InTag,
    Comment(u8),
    Arg(u8),
}

impl TagStrip {
    fn reset(&mut self) {
        self.state = HtmlState::Text;
        self.skip_content = false;
    }

    fn strip_tags(&mut self, txt: &str) -> String {
        let mut out = String::with_capacity(txt.len() / 2);
        let mut tag_name = String::new();
        for ch in txt.chars() {
            match self.state {
                HtmlState::Text => match ch {
                    '<' => {
                        self.state = HtmlState::Lt;
                    }
                    _ => if !self.skip_content {
                        out.push(ch);
                    }
                },
                HtmlState::Lt => match ch {
                    'a'..='z' | 'A'..='Z' => {
                        tag_name.clear();
                        tag_name.push(ch.to_ascii_lowercase());
                        self.state = HtmlState::Name;
                    },
                    '/' => {
                        self.skip_content = false;
                        self.state = HtmlState::InTag;
                    },
                    '!' => {
                        self.state = HtmlState::Comment(0);
                    },
                    _ => {
                        self.state = HtmlState::Text;
                        if !self.skip_content {
                            out.push('<');
                            out.push(ch);
                        }
                    },
                },
                HtmlState::Name => match ch {
                    'a'..='z' | 'A'..='Z' => {
                        tag_name.push(ch.to_ascii_lowercase());
                    },
                    ch => {
                        if tag_name == "style" || tag_name == "script" {
                            self.skip_content = true;
                        }
                        tag_name.clear();
                        self.state = if ch == '>' {  HtmlState::Text } else { HtmlState::InTag };
                    }
                }
                HtmlState::InTag => match ch {
                    '"' | '\'' => {
                        self.state = HtmlState::Arg(ch as u8);
                    },
                    '>' => {
                        self.state = HtmlState::Text;
                    },
                    _ => {},
                }

                HtmlState::Comment(n) => match ch {
                    '-' => {
                        self.state = HtmlState::Comment((n + 1).min(2));
                    },
                    '>' if n >= 2 => {
                        self.state = HtmlState::Text;
                    },
                    _ => {},
                },
                HtmlState::Arg(q) => match ch {
                    '"' | '\'' if (ch as u8) == q => {
                        self.state = HtmlState::InTag;
                    },
                    _ => {},
                }
            }
        }
        out
    }
}

#[test]
fn tags() {
    let mut t = TagStrip {
        skip_content: false,
        state: HtmlState::Text,
    };
    assert_eq!("hi c X aaa 1 <> 2 end", t.strip_tags("<x>hi</x> <!-- com -->c<!--> X<x/> <a href=''>aaa</a> 1 <> 2 <script> garbage <!--> </script>end"));
}

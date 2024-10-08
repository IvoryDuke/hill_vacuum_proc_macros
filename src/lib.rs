#![allow(clippy::single_match_else)]

//=======================================================================//
// IMPORTS
//
//=======================================================================//

use std::{
    fs::File,
    io::{BufRead, BufReader}
};

use hill_vacuum_shared::{
    continue_if_no_match,
    match_or_panic,
    return_if_no_match,
    ManualItem,
    NextValue,
    TEXTURE_HEIGHT_RANGE
};
use proc_macro::{Ident, TokenStream, TokenTree};

//=======================================================================//
// FUNCTIONS
//
//=======================================================================//

/// Checks whever `value` is a comma.
/// # Panics
/// Function panics if `value` is not a comma.
#[inline]
fn is_comma(value: TokenTree)
{
    assert!(match_or_panic!(value, TokenTree::Punct(p), p).as_char() == ',');
}

//=======================================================================//

/// Executes `f` for each Ident contained in `group`'s stream.
/// # Panics
/// Panics if `group` is not a `TokenTree::Group(_)`.
fn for_each_ident_in_group<F: FnMut(Ident)>(group: TokenTree, mut f: F)
{
    for ident in match_or_panic!(group, TokenTree::Group(g), g)
        .stream()
        .into_iter()
        .filter_map(|item| return_if_no_match!(item, TokenTree::Ident(ident), Some(ident), None))
    {
        f(ident);
    }
}

//=======================================================================//

/// Extracts the name of an enum for `iter`.
/// # Panics
/// Panics if `iter` does not belong to an enum.
#[inline]
#[must_use]
fn enum_ident(iter: &mut impl Iterator<Item = TokenTree>) -> Ident
{
    for item in iter.by_ref()
    {
        let ident = continue_if_no_match!(item, TokenTree::Ident(ident), ident);

        if &ident.to_string() == "enum"
        {
            return match_or_panic!(iter.next_value(), TokenTree::Ident(i), i);
        }
    }

    panic!();
}

//=======================================================================//

/// Implements a constant representing the size of the `input` enum.

#[proc_macro_derive(EnumSize)]
#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn enum_size(input: TokenStream) -> TokenStream
{
    let mut iter = input.into_iter();
    format!(
        "impl {} {{ pub const SIZE: usize = {}; }}",
        enum_ident(&mut iter),
        enum_len(iter)
    )
    .parse()
    .unwrap()
}

//=======================================================================//

/// Returns the amount of elements in an enum.
#[allow(clippy::missing_panics_doc)]
#[inline]
#[must_use]
fn enum_len(mut iter: impl Iterator<Item = TokenTree>) -> usize
{
    let mut i = 0;
    for_each_ident_in_group(iter.next_value(), |_| i += 1);
    i
}

//=======================================================================//

/// Implements From `usize` for a plain enum.
/// # Panics
/// Panics if `input` does not belong to an enum.
#[proc_macro_derive(EnumFromUsize)]
#[must_use]
pub fn enum_from_usize(input: TokenStream) -> TokenStream
{
    let mut iter = input.into_iter();
    let enum_ident = enum_ident(&mut iter).to_string();

    let mut from_impl = format!(
        "impl From<usize> for {enum_ident}
        {{
            #[inline]
            #[must_use] fn from(value: usize) -> Self
            {{
                match value
                {{
        "
    );

    let mut i = 0;

    for_each_ident_in_group(iter.next_value(), |ident| {
        from_impl.push_str(&format!("{i} => {enum_ident}::{ident},\n"));
        i += 1;
    });

    from_impl.push_str("_ => unreachable!() } } }");
    from_impl.parse().unwrap()
}

//=======================================================================//

/// Implements a method that returns an iterator to the values of a plain enum.
#[proc_macro_derive(EnumIter)]
#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn enum_iter(input: TokenStream) -> TokenStream
{
    let mut iter = input.into_iter();
    let enum_ident = enum_ident(&mut iter).to_string();
    let enum_len = enum_len(iter.clone());
    let mut enum_match = String::new();

    let mut i = 0;
    for_each_ident_in_group(iter.next_value(), |ident| {
        enum_match.push_str(&format!("{i} => Some({enum_ident}::{ident}),\n"));
        i += 1;
    });

    enum_match.push_str("_ => None");

    format!(
        "
        impl {enum_ident}
        {{
            #[inline]
            pub fn iter() -> impl ExactSizeIterator<Item = Self>
            {{
                struct EnumIterator(usize, usize);

                impl ExactSizeIterator for EnumIterator
                {{
                    #[inline]
                    #[must_use]
                    fn len(&self) -> usize {{ self.1 - self.0 }}
                }}

                impl Iterator for EnumIterator
                {{
                    type Item = {enum_ident};

                    #[inline]
                    fn next(&mut self) -> Option<Self::Item>
                    {{
                        let value = match self.0
                        {{
                            {enum_match}
                        }};

                        self.0 += 1;
                        value
                    }}
                }}

                EnumIterator(0, {enum_len})
            }}
        }}
        "
    )
    .parse()
    .unwrap()
}

//=======================================================================//

/// Generates an array of static [`str`] with name, size, and prefix defined in `stream`.
/// # Examples
/// ```
/// str_array(ARRAY, 4, i_);
/// // Equivalent to
/// const ARRAY: [&'static str; 4] = ["i_0", "i_1", "i_2", "i_3"];
/// ```
/// # Panics
/// Panics if `input` is not properly formatted.
#[proc_macro]
pub fn str_array(input: TokenStream) -> TokenStream
{
    let mut iter = input.into_iter();

    let ident = iter.next_value().to_string();
    is_comma(iter.next_value());

    let amount = iter.next_value().to_string().parse::<u16>().unwrap();

    let prefix = if let Some(v) = iter.next()
    {
        is_comma(v);
        let v = iter.next_value();
        assert!(iter.next().is_none());
        v.to_string()
    }
    else
    {
        String::new()
    };

    let mut result = format!("const {ident}: [&'static str; {amount}] = [");

    for i in 0..amount
    {
        result.push_str(&format!("\"{prefix}{i}\", "));
    }

    result.push_str("];");
    result.parse().unwrap()
}

//=======================================================================//

/// Generates the built-in manual from some of the markdown files in the `docs` directory.
#[allow(clippy::missing_panics_doc)]
#[proc_macro]
pub fn generate_manual(_: TokenStream) -> TokenStream
{
    const SHOW_EXPLANATION: &str = "
    use crate::map::editor::state::{ui::{Tool, SubTool}, core::tool::ToolInterface};

    #[inline]
    fn show_explanation<F: FnOnce(&mut egui::Ui)>(ui: &mut egui::Ui, left: F, explanation: &str)
    {
        ui.horizontal_wrapped(|ui| {
            egui_extras::StripBuilder::new(ui)
                .size(egui_extras::Size::exact(250f32))
                .size(egui_extras::Size::remainder())
                .horizontal(|mut strip| {
                    strip.cell(|ui| {
                        left(ui);
                    });

                    strip.cell(|ui| {
                        ui.label(explanation);
                    });
                });
        });
    }";

    let path = std::env::current_dir().unwrap();
    let path = path.to_str().unwrap();

    let body = hill_vacuum_shared::process_docs(
        |string| {
            string.push_str("ui.collapsing(\n");
        },
        |string, name, item| {
            match item
            {
                ManualItem::Regular =>
                {
                    string.push('\"');
                    string.push_str(&name.to_ascii_uppercase());
                    string.push_str("\",\n");
                    string.push_str("|ui| {\nui.vertical(|ui| {\n");
                },
                ManualItem::Tool =>
                {
                    let mut chars = name.chars();
                    let mut tool = chars.next_value().to_ascii_uppercase().to_string();

                    while let Some(mut c) = chars.next()
                    {
                        if c == ' '
                        {
                            c = chars.next_value().to_ascii_uppercase();
                        }

                        tool.push(c);
                    }

                    string.push_str(&format!(
                        "Tool::{tool}.header(),\n|ui| {{\nui.vertical(|ui| \
                         {{\ntools_buttons.image(ui, Tool::{tool});\n"
                    ));
                },
                ManualItem::Texture => unreachable!()
            };
        },
        |string, name, file, item| {
            let processed = file
                .trim()
                .replace("### ", "")
                .replace("```ini", "")
                .replace('\"', "\\\"")
                .replace("   ", "")
                .replace('`', "");

            match item
            {
                ManualItem::Regular =>
                {
                    let mut lines = processed.lines();
                    let command = lines.next_value();
                    let mut exp = String::new();

                    for line in lines
                    {
                        exp.push_str(line);
                        exp.push('\n');
                    }

                    exp.pop();
                    string.push_str(&format!(
                        "show_explanation(ui, |ui| {{ ui.label(\"{command}\"); }}, \"{exp}\");\n"
                    ));
                },
                ManualItem::Tool =>
                {
                    let mut chars = name.chars();
                    let mut subtool = chars.next_value().to_ascii_uppercase().to_string();

                    while let Some(mut c) = chars.next()
                    {
                        if c == '_'
                        {
                            c = chars.next_value().to_ascii_uppercase();
                        }

                        subtool.push(c);
                    }

                    let mut lines = processed.lines();
                    let mut exp = lines.next_value().to_string();
                    exp.push_str(" (");
                    exp.push_str(
                        &std::fs::read_to_string(format!("{path}/docs/subtools binds/{name}.md"))
                            .unwrap()
                    );
                    exp.push_str(")\n");

                    for line in lines
                    {
                        exp.push_str(line);
                        exp.push('\n');
                    }

                    exp.pop();

                    string.push_str(&format!(
                        "show_explanation(ui, |ui| {{ tools_buttons.image(ui, \
                         SubTool::{subtool}); }}, \"{exp}\");\n"
                    ));
                },
                ManualItem::Texture =>
                {
                    string.push_str(&format!(
                        "show_explanation(ui, |ui| {{ ui.label(\"TEXTURE EDITING\"); }}, \
                         \"{processed}\");\n"
                    ));
                }
            };
        },
        |string, last| {
            string.push_str("})\n});\n\n");

            if !last
            {
                string.push_str("ui.separator();\n\n");
            }
        }
    );

    format!("{SHOW_EXPLANATION}\n\n{body}").parse().unwrap()
}

//=======================================================================//

/// Generates a function which associates a f32 value representing a certain height to each provided
/// enum match arm.
#[allow(clippy::missing_panics_doc)]
#[proc_macro]
pub fn color_enum(stream: TokenStream) -> TokenStream
{
    #[inline]
    fn is_column<I: Iterator<Item = TokenTree>>(stream: &mut I)
    {
        assert!(match_or_panic!(stream.next_value(), TokenTree::Punct(p), p).as_char() == ':');
    }

    #[inline]
    fn push_key_and_label(item: &str, label_func: &mut String, key_func: &mut String)
    {
        let mut chars = item.chars();
        let c = chars.next_value();
        key_func.push_str(&format!("Self::{item} => \"{}", c.to_ascii_lowercase()));
        label_func.push_str(&format!("Self::{item} => \"{c}"));

        for c in chars
        {
            if c.is_uppercase()
            {
                key_func.push('_');
                key_func.push(c.to_ascii_lowercase());

                label_func.push(' ');
                label_func.push(c);

                continue;
            }

            for func in [&mut *key_func, &mut *label_func]
            {
                func.push(c);
            }
        }

        for func in [key_func, label_func]
        {
            func.push_str("\",\n");
        }
    }

    #[inline]
    #[must_use]
    fn extract<I: Iterator<Item = TokenTree>>(
        stream: &mut I,
        end_tag: &str,
        label_func: &mut String,
        key_func: &mut String
    ) -> Vec<String>
    {
        let mut vec: Vec<String> = Vec::new();

        while let Some(item) = stream.next()
        {
            if let TokenTree::Punct(p) = item
            {
                let c = p.as_char();

                match c
                {
                    ',' => (),
                    '|' =>
                    {
                        let last = vec.last_mut().unwrap();
                        let item = stream.next_value().to_string();
                        push_key_and_label(&item, label_func, key_func);
                        last.push_str(&format!(" | Self::{item}"));
                    },
                    _ => panic!()
                }

                continue;
            }

            let item = item.to_string();

            if item == end_tag
            {
                is_column(stream);
                break;
            }

            push_key_and_label(&item, label_func, key_func);
            vec.push(format!("Self::{item}"));
        }

        vec
    }

    #[inline]
    #[must_use]
    fn generate_height_func<'a, I: Iterator<Item = &'a str>>(
        start: &str,
        mut start_height: f32,
        interval: f32,
        iter: I
    ) -> (String, f32)
    {
        let mut height_func = start.to_string();

        for item in iter
        {
            height_func.push_str(&format!("{item} => {start_height}f32,\n"));
            start_height += interval;
        }

        height_func.push_str("_ => panic!(\"Invalid color: {self:?}\")\n}\n}");
        (height_func, start_height)
    }

    let textures_interval = f32::from(*TEXTURE_HEIGHT_RANGE.end());
    let mut stream = stream.into_iter();

    let mut key_func = "
    /// The config file key relative to the drawn color associated with [`Color`].
    #[inline]
    #[must_use]
    pub const fn config_file_key(self) -> &'static str
    {
        match self
        {
    "
    .to_string();

    let mut label_func = "
    /// The text label representing [`Color`] in UI elements.
    #[inline]
    #[must_use]
    pub const fn label(self) -> &'static str
    {
        match self
        {
    "
    .to_string();

    assert!(stream.next_value().to_string() == "clear");
    is_column(&mut stream);
    let clear = stream.next_value().to_string();
    push_key_and_label(&clear, &mut label_func, &mut key_func);
    is_comma(stream.next_value());

    assert!(stream.next_value().to_string() == "extensions");
    is_column(&mut stream);
    let extensions = stream.next_value().to_string();
    push_key_and_label(&extensions, &mut label_func, &mut key_func);
    let extensions = format!("Self::{extensions}");
    is_comma(stream.next_value());

    assert!(stream.next_value().to_string() == "grid");
    is_column(&mut stream);
    let grid = extract(&mut stream, "entities", &mut label_func, &mut key_func);
    let entities = extract(&mut stream, "ui", &mut label_func, &mut key_func);
    let ui = extract(&mut stream, "", &mut label_func, &mut key_func);

    for func in [&mut key_func, &mut label_func]
    {
        func.push_str("}\n}");
    }

    let (height_func, clip_height) = generate_height_func(
        "
    /// The height at which map elements colored with a certain [`Color`] should be drawn.
    #[inline]
    #[must_use]
    pub fn entity_height(self) -> f32
    {
        match self
        {",
        1f32,
        textures_interval + 1f32,
        entities.iter().map(String::as_str)
    );

    let (line_height_func, thing_angle_height) = generate_height_func(
        "
    /// The draw height of the lines.
    #[inline]
    #[must_use]
    pub fn line_height(self) -> f32
    {
        match self
        {
    ",
        clip_height + 1f32,
        1f32,
        grid.iter()
            .chain(Some(&extensions).iter().copied())
            .chain(&entities)
            .chain(&ui)
            .map(String::as_str)
    );

    let (square_hgl_height_func, _) = generate_height_func(
        "
    /// The draw height of the square highlights.
    #[inline]
    #[must_use]
    pub fn square_hgl_height(self) -> f32
    {
        match self
        {
    ",
        thing_angle_height + 2f32,
        1f32,
        ui.iter().map(String::as_str)
    );

    format!(
        "
    {height_func}

    /// The draw height of an untextured polygon.
    #[inline]
    #[must_use]
    pub(in crate::map::drawer) fn polygon_height(self) -> f32 {{ self.entity_height() - 1f32 }}

    /// The draw height of the clip overlay.
    #[inline]
    #[must_use]
    pub(in crate::map::drawer) const fn clip_height() -> f32 {{ {clip_height}f32 }}

    {line_height_func}

    /// The draw height of the thing angle indicator.
    #[inline]
    #[must_use]
    pub(in crate::map::drawer) fn thing_angle_indicator_height() -> [f32; 2]
    {{
        [
            {thing_angle_height}f32,
            {thing_angle_height}f32 + 1f32
        ]
    }}

    {square_hgl_height_func}

    {key_func}

    {label_func}"
    )
    .parse()
    .unwrap()
}

//=======================================================================//

/// Generates the `Bind` enum plus the `config_file_key()` and `label()` methods.
/// # Panics
/// Panic if the file containing the `Tool` enum is not at the required location.
#[proc_macro]
pub fn bind_enum(input: TokenStream) -> TokenStream
{
    let mut binds = "{".to_string();
    binds.push_str(&input.to_string());
    binds.push(',');

    let mut path = std::env::current_dir().unwrap();
    path.push("src/map/editor/state/core/tool.rs");

    let mut lines = BufReader::new(File::open(path).unwrap()).lines().map(Result::unwrap);
    lines.find(|line| line.ends_with("enum Tool"));
    lines.next();

    for line in lines
    {
        binds.push_str(&line);
        binds.push('\n');

        if line.contains('}')
        {
            break;
        }
    }

    let mut iter = binds.clone().parse::<TokenStream>().unwrap().into_iter();

    let mut key_func = "
    /// Returns the string key used in the config file associated with this `Bind`. 
    #[inline]
    #[must_use]
    pub(in crate::config::controls) const fn config_file_key(self) -> &'static str
    {
        match self
        {\n"
    .to_string();

    let mut label_func = "
    /// Returns the text representing this `Bind` in UI elements.
    #[inline]
    #[must_use]
    pub const fn label(self) -> &'static str
    {
        match self
        {\n"
    .to_string();

    for item in match_or_panic!(iter.next_value(), TokenTree::Group(g), g).stream()
    {
        if let TokenTree::Ident(ident) = item
        {
            let ident = ident.to_string();
            let mut chars = ident.chars();
            let mut value = chars.next_value().to_string();

            for ch in chars
            {
                if ch.is_ascii_uppercase()
                {
                    value.push(' ');
                }

                value.push(ch);
            }

            label_func.push_str(&format!("Self::{ident} => \"{value}\",\n"));

            value = value.to_ascii_lowercase().replace(' ', "_");
            key_func.push_str(&format!("Self::{ident} => \"{value}\",\n"));
        }
    }

    for func in [&mut key_func, &mut label_func]
    {
        func.push_str("}\n}");
    }

    format!(
        "
        /// The binds associated with the editor actions.
        #[derive(Clone, Copy, Debug, PartialEq, EnumIter, EnumSize)]
        pub enum Bind
        {binds}

        impl Bind
        {{
            {key_func}

            {label_func}
        }}"
    )
    .parse()
    .unwrap()
}

//=======================================================================//

/// Generates the `header()` and `icon_file_name()` methods for the `Tool` and `SubTool` enums.
#[inline]
#[must_use]
fn tools_common(stream: TokenStream, id: &str) -> [String; 2]
{
    let mut header_func = "
        /// The uppercase tool name.
        #[inline]
        #[must_use]
        fn header(self) -> &'static str
        {
            match self
            {\n"
    .to_string();

    let mut icon_file_name_func = "
        /// The file name of the associated icon.
        #[inline]
        #[must_use]
        fn icon_file_name(self) -> &'static str
        {
            match self
            {\n"
    .to_string();

    for item in stream
    {
        let ident = continue_if_no_match!(item, TokenTree::Ident(ident), ident).to_string();
        let mut chars = ident.chars();

        // Label.
        let mut value = chars.next_value().to_string();

        for ch in chars
        {
            if ch.is_ascii_uppercase()
            {
                value.push(' ');
            }

            value.push(ch);
        }

        // Header.
        value = value.to_ascii_uppercase();
        header_func.push_str(&format!("Self::{ident} => \"{value} {id}\",\n"));

        // Icon paths.
        value = value.to_ascii_lowercase().replace(' ', "_");
        icon_file_name_func.push_str(&format!("Self::{ident} => \"{value}.png\",\n"));
    }

    for func in [&mut icon_file_name_func, &mut header_func]
    {
        func.push_str("}\n}");
    }

    [header_func, icon_file_name_func]
}

//=======================================================================//

/// Implements the vast majority of the methods of the `Tool` enum.
/// # Panics
/// Panics if `input` does not belong to the `Tool` enum.
#[proc_macro_derive(ToolEnum)]
#[must_use]
pub fn declare_tool_enum(input: TokenStream) -> TokenStream
{
    let mut iter = input.into_iter();
    assert!(enum_ident(&mut iter).to_string() == "Tool");
    let group = match_or_panic!(iter.next_value(), TokenTree::Group(g), g);
    let [header_func, icon_file_name_func] = tools_common(group.stream(), "TOOL");

    let mut bind_func = "#[inline]
        pub const fn bind(self) -> Bind
        {
            match self
            {\n"
    .to_string();

    let mut label_func = "#[inline]
        fn label(self) -> &'static str
        {
            match self
            {\n"
    .to_string();

    for item in group.stream()
    {
        let ident = continue_if_no_match!(item, TokenTree::Ident(ident), ident).to_string();
        let mut chars = ident.chars();

        // Bind
        bind_func.push_str(&format!("Self::{ident} => Bind::{ident},\n"));

        // Label.
        let mut value = chars.next_value().to_string();

        for ch in chars
        {
            if ch.is_ascii_uppercase()
            {
                value.push(' ');
            }

            value.push(ch);
        }

        label_func.push_str(&format!("Self::{ident} => \"{value}\",\n"));
    }

    for func in [&mut label_func, &mut bind_func]
    {
        func.push_str("}\n}");
    }

    format!(
        "
        impl ToolInterface for Tool
        {{
            {label_func}

            {header_func}

            {icon_file_name_func}

            #[inline]
            fn tooltip_label(self, binds: &BindsKeyCodes) -> String
            {{
                format!(\"{{}} ({{}})\", self.label(), self.keycode_str(binds))
            }}

            #[inline]
            fn change_conditions_met(self, change_conditions: &ChangeConditions) -> bool
            {{
                self.conditions_met(change_conditions)
            }}

            #[inline]
            fn subtool(self) -> bool {{ false }}

            #[inline]
            fn index(self) -> usize {{ self as usize }}
        }}

        impl Tool
        {{
            {bind_func}
        }}"
    )
    .parse()
    .unwrap()
}

//=======================================================================//

/// Implements the vast majority of the methods of the `SubTool` enum.
/// # Panics
/// Panics if `input` does not belong to the `SubTool` enum.
#[proc_macro_derive(SubToolEnum)]
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn subtool_enum(input: TokenStream) -> TokenStream
{
    let mut iter = input.into_iter();
    assert!(enum_ident(&mut iter).to_string() == "SubTool");
    let group = match_or_panic!(iter.next_value(), TokenTree::Group(g), g);
    let [header_func, icon_file_name_func] = tools_common(group.stream(), "SUBTOOL");

    let mut label_func = "
        #[inline]
        fn label(self) -> &'static str
        {
            match self
            {\n"
    .to_string();

    let mut bind_func = "
        #[inline]
        fn bind(self) -> &'static str
        {
            match self
            {\n"
    .to_string();

    let mut tool_func = "
        #[inline]
        const fn tool(self) -> Tool
        {
            match self
            {\n"
    .to_string();

    let mut tool = String::new();
    let mut label = String::new();
    let mut bind = String::new();
    let mut subtool_binds_path = std::env::current_dir().unwrap();
    subtool_binds_path.push("docs");
    subtool_binds_path.push("subtools binds");

    for item in group.stream()
    {
        let ident = continue_if_no_match!(item, TokenTree::Ident(ident), ident).to_string();
        let mut chars = ident.chars();
        let first = chars.next_value();

        for s in [&mut tool, &mut label, &mut bind]
        {
            s.clear();
        }

        tool.push(first);
        bind.push(first.to_ascii_lowercase());

        for ch in chars.by_ref()
        {
            if ch.is_ascii_uppercase()
            {
                label.push(ch);

                bind.push('_');
                bind.push(ch.to_ascii_lowercase());
                break;
            }

            tool.push(ch);
            bind.push(ch);
        }

        for ch in chars
        {
            if ch.is_ascii_uppercase()
            {
                label.push(' ');
                bind.push('_');
            }

            label.push(ch);
            bind.push(ch.to_ascii_lowercase());
        }

        subtool_binds_path.push(format!("{bind}.md"));

        label_func.push_str(&format!("Self::{ident} => \"{label}\",\n"));
        tool_func.push_str(&format!("Self::{ident} => Tool::{tool},\n"));
        bind_func.push_str(&format!("Self::{ident} => include_str!({:?}),\n", subtool_binds_path));

        subtool_binds_path.pop();
    }

    for func in [&mut label_func, &mut tool_func, &mut bind_func]
    {
        func.push_str("}\n}");
    }

    format!(
        "
        impl ToolInterface for SubTool
        {{
            {label_func}

            {header_func}

            {icon_file_name_func}

            #[inline]
            fn tooltip_label(self, _: &BindsKeyCodes) -> String
            {{
                format!(\"{{}} ({{}})\", self.label(), self.bind())
            }}

            #[inline]
            fn change_conditions_met(self, change_conditions: &ChangeConditions) -> bool
            {{
                self.conditions_met(change_conditions)
            }}

            #[inline]
            fn subtool(self) -> bool {{ true }}

            #[inline]
            fn index(self) -> usize {{ self as usize }}
        }}

        impl SubTool
        {{
            {tool_func}

            {bind_func}
        }}
        "
    )
    .parse()
    .unwrap()
}

//=======================================================================//

/// Generates the function calls to store the embedded assets from the file names in the
/// `src/embedded_assets/` folder.
/// # Panics
/// Panics if the required folder cannot be found.
#[allow(clippy::missing_panics_doc)]
#[proc_macro]
pub fn embedded_assets(_: TokenStream) -> TokenStream
{
    let mut path = std::env::current_dir().unwrap();
    path.push("src/embedded_assets/");

    // Get all the files.
    let directory = std::fs::read_dir(path).unwrap();
    let mut values = String::new();
    values.push_str("use bevy::asset::embedded_asset;\n");

    for file in directory.into_iter().map(|p| p.unwrap().file_name())
    {
        let file_name = file.to_str().unwrap();
        values.push_str(&format!("bevy::asset::embedded_asset!(app, \"{file_name}\");\n"));
    }

    values.parse().unwrap()
}

//=======================================================================//

/// Generates the vector of the indexes used to triangulate the meshes.
#[allow(clippy::missing_panics_doc)]
#[proc_macro]
pub fn meshes_indexes(stream: TokenStream) -> TokenStream
{
    let mut stream = stream.into_iter();
    let ident = stream.next_value().to_string();
    is_comma(stream.next_value());
    let size = stream.next_value().to_string().parse::<u16>().unwrap();
    assert!(stream.next().is_none());

    let mut indexes = format!(
        "
    const MAX_MESH_TRIANGLES: usize = {size};
    static mut {ident}: *mut [u16] = &mut [\n"
    );

    for i in 1..=size
    {
        indexes.push_str(&format!("0u16, {i}, {i} + 1,\n"));
    }

    indexes.push_str("];");
    indexes.parse().unwrap()
}

//=======================================================================//

/// Generates the sin, cos, tan, lookup table.
#[allow(clippy::cast_precision_loss)]
#[allow(clippy::missing_panics_doc)]
#[proc_macro]
pub fn sin_cos_tan_array(_: TokenStream) -> TokenStream
{
    let mut array = "
    #[allow(clippy::approx_constant)]
    #[allow(clippy::unreadable_literal)]
    const SIN_COS_TAN_LOOKUP: [(f32, f32, f32); 361] = [\n"
        .to_string();

    for a in 0..=360
    {
        let a = (a as f32).to_radians();
        array.push_str(&format!("({}f32, {}f32, {}f32),\n", a.sin(), a.cos(), a.tan()));
    }

    array.push_str("];");
    array.parse().unwrap()
}

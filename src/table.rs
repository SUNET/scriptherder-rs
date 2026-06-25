use regex::Regex;

#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Right,
}

pub struct ColumnMeta {
    pub name: String,
    pub width: usize,
    pub align: Align,
}

impl ColumnMeta {
    pub fn new(name: &str, align: Align) -> ColumnMeta {
        let mut m = ColumnMeta {
            name: name.to_string(),
            width: 0,
            align,
        };
        m.update_width(name.chars().count());
        m
    }

    pub fn update_width(&mut self, value: usize) {
        if value > self.width {
            self.width = value;
        }
    }

    /// Format `value` (with known `print_width`) padded to column width.
    pub fn format(&self, value: &str, print_width: usize) -> String {
        let pad = " ".repeat(self.width.saturating_sub(print_width));
        if self.align == Align::Right {
            format!("{pad}{value}")
        } else {
            format!("{value}{pad}")
        }
    }
}

pub struct DataTable {
    rows: Vec<Vec<(String, usize)>>,
    curr: Vec<(String, usize)>,
    meta: Vec<ColumnMeta>,
    ansi: Regex,
}

impl DataTable {
    pub fn new(meta: Vec<ColumnMeta>) -> DataTable {
        DataTable {
            rows: vec![],
            curr: vec![],
            meta,
            ansi: Regex::new(r"\x1b\[[;\d]*[A-Za-z]").unwrap(),
        }
    }

    pub fn push(&mut self, value: &str) {
        let print_width = self.ansi.replace_all(value, "").chars().count();
        self.curr.push((value.to_string(), print_width));
        if self.meta.len() >= self.curr.len() {
            let idx = self.curr.len() - 1;
            self.meta[idx].update_width(print_width);
        }
    }

    pub fn new_line(&mut self) {
        self.rows.push(std::mem::take(&mut self.curr));
    }

    /// Render the table as a string (equivalent to Python `__str__`).
    pub fn render(&self) -> String {
        let mut res: Vec<String> = Vec::new();

        // Header row: field names
        let mut header = String::new();
        for h in &self.meta {
            let w = h.name.chars().count();
            header.push_str(&h.format(&h.name, w));
            header.push_str("  ");
        }
        res.push(header.trim_end().to_string());

        // Data rows
        for row in &self.rows {
            let mut line = String::new();
            for (idx, (value, pw)) in row.iter().enumerate() {
                if self.meta.len() > idx {
                    line.push_str(&self.meta[idx].format(value, *pw));
                } else {
                    line.push_str(value);
                }
                line.push_str("  ");
            }
            res.push(line.trim_end().to_string());
        }

        res.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_columns_and_strips_ansi_for_width() {
        let meta = vec![
            ColumnMeta::new("Name", Align::Left),
            ColumnMeta::new("N", Align::Right),
        ];
        let mut t = DataTable::new(meta);
        t.push("alpha");
        t.push("1");
        t.new_line();
        t.push("\u{1b}[1mb\u{1b}[0m");
        t.push("22");
        t.new_line();
        let out = t.render();
        let lines: Vec<&str> = out.lines().collect();
        // header: "Name" left-aligned to width 5 (from "alpha"), "N" right-aligned to width 2 (from "22")
        // "Name " + "  " + " N" -> rstripped = "Name    N"
        assert_eq!(lines[0], "Name    N");
        // row 1: "alpha" (width 5) + "  " + " 1" (right, width 2) -> "alpha   1"
        assert!(lines[1].ends_with(" 1"));
        // row 2: colored "b" (print width 1, padded to 5) + "  " + "22" (right, width 2)
        assert!(lines[2].starts_with("\u{1b}[1mb\u{1b}[0m"));
        assert!(lines[2].ends_with("22"));
    }
}

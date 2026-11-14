use nodex_lang_rust::lower_source;

#[test]
fn signed_widths_and_ranges_are_visible_on_declarations() {
    let text = "struct S { a: i8, b: i16 } fn f(x: i32) -> i64 { let ignored: u32 = 1; 0 } type A = Option<i8>;";
    let ir = lower_source("src/lib.rs", text);
    for (symbol, ty, min, max) in [("S", "i8", "-128", "127"), ("S", "i16", "-32,768", "32,767"), ("f", "i32", "-2,147,483,648", "2,147,483,647"), ("f", "i64", "-9,223,372,036,854,775,808", "9,223,372,036,854,775,807"), ("A", "i8", "-128", "127")] {
        let symbol = ir.symbols.iter().find(|s| s.name == symbol).unwrap();
        let contract = symbol.rust_contract.as_ref().unwrap();
        let item = contract.iter().find(|item| item.group == "Signed integer type" && &text[item.span.start_byte as usize..item.span.end_byte as usize] == ty).unwrap();
        assert!(item.text.contains(min) && item.text.contains(max), "{}", item.text);
        assert!(item.text.contains("inclusive"));
    }
}

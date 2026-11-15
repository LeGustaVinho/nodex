use nodex_lang_rust::lower_source;

#[test]
fn packed_two_explains_reduced_placement_alignment_and_unaligned_reference_hazards() {
    let source = "#[repr(packed(2))] struct A { a: u8, b: u32 } #[repr(C, packed(2))] struct B { a: u8, b: u32 } #[repr(packed(2))] union U { a: u8, b: u32 } #[repr(packed(4))] struct Four { a: u32 } #[repr(align(2))] struct Aligned { a: u8 } #[cfg_attr(feature = \"x\", repr(packed(2)))] struct Conditional { a: u32 }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["A", "B", "U"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Two-byte packed representation").collect();
        assert_eq!(facts.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = facts[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "packed(2)");
        assert!(item.text.contains("smaller of that field type's alignment and 2 bytes"));
        assert!(item.text.contains("does not necessarily remove all padding"));
        assert!(item.text.contains("does not guarantee field order"));
        assert!(item.text.contains("unaligned & or &mut reference is invalid"));
        assert!(item.text.contains("raw pointer without first creating a reference"));
        assert!(item.text.contains("does not prove a particular field offset"));
    }
    for name in ["Four", "Aligned", "Conditional"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Two-byte packed representation"), "{name}: {:?}", symbol.rust_contract);
    }
}

use nodex_lang_rust::lower_source;

#[test]
fn packed_representation_explains_alignment_padding_and_reference_safety_on_type_and_fields() {
    let source = r#"
        #[repr(packed)] struct Packed { byte: u8, word: u32 }
        #[repr(C, packed)] union Combined { i: u32, b: u8 }
        #[repr(packed(1))] struct Tuple(u8, u32);
        #[repr(packed(2))] struct Different { x: u32 }
        #[repr(C)] struct Plain { x: u32 }
        #[cfg_attr(test, repr(packed))] struct Conditional { x: u32 }
    "#;
    let ir = lower_source("src/lib.rs", source);
    for name in ["Packed", "Combined", "Tuple"] {
        let owner = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let items: Vec<_> = owner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Packed representation").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", owner.rust_contract);
        assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "packed");
        for phrase in ["alignment is one byte", "no padding between", "field's own type", "does not guarantee field order", "unaligned reference", "undefined behavior", "&raw const", "read_unaligned", "unsafe validity and ownership"] {
            assert!(items[0].text.contains(phrase), "{name}: {}", items[0].text);
        }
        let fields: Vec<_> = ir.symbols.iter().filter(|symbol| symbol.parent.as_deref() == Some(owner.id.as_str())).collect();
        assert!(!fields.is_empty(), "{name}: no fields");
        for field in fields {
            assert!(field.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Packed representation"), "{name}::{}: {:?}", field.name, field.rust_contract);
        }
    }
    for name in ["Different", "Plain", "Conditional"] {
        let owner = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!owner.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Packed representation"), "{name}");
    }
}

//! # Nodex Rust frontend
//!
//! Lowers Rust (`.rs`) to the Nodex IR using tree-sitter: modules, types, traits and `impl`
//! blocks become structure symbols, function bodies become Blueprints with the shared
//! vocabulary. Data wires carry Rust ownership (move, copy, borrow). Semantics come from
//! rust-analyzer ([`semantic`]); live traces from an instrumented copy of the crate
//! ([`instrument`]).

mod borrow;
mod unsafe_c_fn_pointer_detail;
mod default_extern_abi_detail;
mod foreign_static_mut_detail;
mod decimal_separators_detail;
mod binary_separators_detail;
mod named_c_variadic_detail;
mod char_escapes_detail;
mod exponent_suffix_detail;
mod raw_keyword_detail;
mod string_continuation_detail;
mod qualified_assoc_call_explanation;
mod generic_type_args_explanation;
mod inferred_const_args_explanation;
mod constructed_where_explanation;
mod higher_rank_where_explanation;
mod associated_bound_explanation;
mod independent_impl_args_explanation;
mod opaque_inferred_lifetime_explanation;
mod empty_capture_explanation;
mod named_fn_pointer_explanation;
mod expr;
mod external_module_reading;
mod flow;
mod debug;
pub mod instrument;
mod knowledge;
pub mod project;
mod reading;
pub mod semantic;
mod structure;
mod syntax;
mod shadow_reading;
mod reading_tuple_let;
mod struct_binding_reading;
mod let_else_explanation;
mod const_explanation;
mod static_explanation;
mod static_mut_explanation;
mod function_explanation;
mod function_signature_explanation;
mod never_return_reading;
mod pub_function_explanation;
mod inline_module_explanation;
mod import_item_explanation;
mod import_path_explanation;
mod grouped_import_explanation;
mod import_alias_explanation;
mod import_self_group_explanation;
mod glob_import_explanation;
mod public_reexport_explanation;
mod extern_crate_explanation;
mod public_visibility_explanation;
mod pub_crate_explanation;
mod pub_super_explanation;
mod pub_self_explanation;
mod pub_in_explanation;
mod crate_path_explanation;
mod self_path_explanation;
mod super_path_explanation;
mod path_separator_explanation;
mod self_type_explanation;
mod raw_identifier_explanation;
mod absolute_path_explanation;
mod super_single_path_explanation;
mod self_explanation;
mod cast_as_explanation;
mod named_struct_explanation;
mod tuple_struct_explanation;
mod record_construct_explanation;
mod struct_shorthand_reading;
mod newtype_explanation;
mod raw_lifetime_label_explanation;
mod unit_struct_explanation;
mod tuple_struct_call_explanation;
mod unit_enum_explanation;
mod enum_tuple_payload_explanation;
mod named_enum_variant_explanation;
mod enum_named_construction_explanation;
mod union_declaration_explanation;
mod trait_declaration_explanation;
mod tuple_variant_call_explanation;
mod union_initialization_explanation;
mod enum_discriminant_explanation;
mod supertrait_explanation;
mod multiple_supertraits_explanation;
mod shared_receiver_explanation;
mod new_associated_function_explanation;
mod blanket_impl_explanation;
mod trait_impl_explanation;
mod generic_inherent_impl_explanation;
mod inherent_impl_explanation;
mod associated_type_impl_explanation;
mod trait_associated_type_explanation;
mod default_trait_method_explanation;
mod unsafe_trait_explanation;
mod required_associated_const_explanation;
mod boxed_receiver_explanation;
mod arc_receiver_explanation;
mod unsafe_impl_explanation;
mod type_alias_explanation;
mod add_impl_explanation;
mod pointer_sized_integer_explanation;
mod str_type_explanation;
mod unit_type_explanation;
mod unsigned_integer_type_explanation;
mod tuple_type_explanation;
mod array_type_explanation;
mod array_literal_explanation;
mod array_repeat_beginner_explanation;
mod tuple_field_access_explanation;
mod single_tuple_value_explanation;
mod f32_literal_explanation;
mod f64_literal_explanation;
mod unicode_char_escape_explanation;
mod char_hex_escape_explanation;
mod string_unicode_escape_explanation;
mod raw_c_string_explanation;
mod outer_block_doc_explanation;
mod inner_block_doc_explanation;
mod raw_c_string_hash_explanation;
mod inner_line_doc_explanation;
mod outer_line_doc_explanation;
mod c_string_literal_explanation;
mod macro_rules_definition_explanation;
mod braced_macro_invocation_explanation;
mod macro_paren_invocation_explanation;
mod shebang_explanation;
mod nested_block_comment_explanation;
mod bracket_macro_invocation_explanation;
mod macro_parenthesized_rule_explanation;
mod macro_expression_rule_explanation;
mod macro_square_transcriber_explanation;
mod qualified_macro_invocation_explanation;
mod macro_expr_fragment_explanation;
mod macro_block_fragment_explanation;
mod macro_item_fragment_explanation;
mod macro_lifetime_fragment_explanation;
mod macro_pat_param_explanation;
mod macro_ident_fragment_explanation;
mod macro_pattern_fragment_explanation;
mod macro_expr_2021_fragment_explanation;
mod macro_meta_fragment_explanation;
mod macro_literal_fragment_explanation;
mod macro_path_fragment_explanation;
mod macro_stmt_fragment_explanation;
mod macro_tt_fragment_explanation;
mod macro_type_fragment_explanation;
mod macro_visibility_fragment_explanation;
mod macro_transcriber_metavariable_explanation;
mod macro_optional_tt_explanation;
mod macro_semicolon_repeat_explanation;
mod macro_comma_plus_expr_explanation;
mod macro_expr_repetition_explanation;
mod proc_macro_attribute_explanation;
mod macro_dollar_crate_explanation;
mod proc_macro_derive_explanation;
mod derive_clone_debug_explanation;
mod custom_outer_attribute_explanation;
mod vec_list_macro_explanation;
mod dbg_macro_explanation;
mod debug_format_macro_explanation;
mod proc_macro_function_explanation;
mod println_positional_explanation;
mod option_env_macro_explanation;
mod env_macro_explanation;
mod unimplemented_macro_explanation;
mod cfg_linux_macro_explanation;
mod matches_macro_conditional_explanation;
mod todo_macro_explanation;
mod include_str_macro_explanation;
mod panic_message_macro_explanation;
mod include_bytes_macro_explanation;
mod compile_error_macro_explanation;
mod unreachable_macro_conditional_explanation;
mod generic_outer_attribute_explanation;
mod unsafe_no_mangle_explanation;
mod attribute_name_value_explanation;
mod cfg_target_arch_explanation;
mod inner_generic_attribute_explanation;
mod attribute_nested_value_explanation;
mod cfg_target_os_explanation;
mod cfg_all_explanation;
mod cfg_feature_explanation;
mod ignore_test_attribute_explanation;
mod test_attribute_explanation;
mod should_panic_attribute_explanation;
mod cfg_any_explanation;
mod cfg_not_explanation;
mod derive_clone_attribute_explanation;
mod derive_copy_clone_explanation;
mod cfg_attr_derive_explanation;
mod derive_debug_attribute_explanation;
mod doc_hidden_explanation;
mod doc_value_explanation;
mod diagnostic_on_unimplemented_explanation;
mod rustfmt_skip_explanation;
mod link_name_library_explanation;
mod must_use_message_explanation;
mod clippy_tool_attribute_explanation;
mod diagnostic_do_not_recommend_explanation;
mod must_use_attribute_explanation;
mod doc_alias_explanation;
mod repr_transparent_explanation;
mod repr_align_sixteen_explanation;
mod link_name_symbol_explanation;
mod repr_u8_enum_explanation;
mod repr_c_explanation;
mod link_static_library_explanation;
mod no_link_explanation;
mod link_ordinal_explanation;
mod repr_packed_two_explanation;
mod repr_packed_explanation;
mod used_static_explanation;
mod crate_type_cdylib_explanation;
mod inline_hint_explanation;
mod crate_name_attribute_explanation;
mod no_mangle_linking_explanation;
mod crate_type_rlib_explanation;
mod no_main_attribute_explanation;
mod repr_c_u8_explanation;
mod unsafe_export_name_explanation;
mod unsafe_link_section_explanation;
mod inline_never_explanation;
mod cold_attribute_explanation;
mod no_builtins_explanation;
mod track_caller_explanation;
mod inline_always_explanation;
mod unsafe_naked_explanation;
mod target_feature_avx2_explanation;
mod no_std_explanation;
mod no_implicit_prelude_explanation;
mod arm_instruction_set_explanation;
mod non_exhaustive_struct_explanation;
mod panic_handler_explanation;
mod non_exhaustive_enum_explanation;
mod windows_subsystem_explanation;
mod global_allocator_explanation;
mod recursion_limit_explanation;
mod debugger_gdb_script_explanation;
mod type_length_limit_explanation;
mod collapse_debuginfo_yes_explanation;
mod debugger_natvis_explanation;
mod collapse_debuginfo_external_explanation;
mod collapse_debuginfo_no_explanation;
mod extern_c_fn_explanation;
mod extern_system_fn_explanation;
mod foreign_explicit_unsafe_fn_explanation;
mod foreign_static_default_explanation;
mod unsafe_extern_c_block_explanation;
mod foreign_safe_fn_explanation;
mod foreign_safe_static_explanation;
mod foreign_fn_default_unsafe_explanation;
mod extern_c_unwind_explanation;
mod extern_system_unwind_explanation;
mod extern_custom_abi_explanation;
mod extern_rust_fn_explanation;
mod extern_win64_fn_explanation;
mod extern_cdecl_fn_explanation;
mod extern_stdcall_fn_explanation;
mod extern_c_fn_pointer_explanation;
mod foreign_c_variadic_explanation;
mod extern_sysv64_fn_explanation;
mod c_variadic_definition_explanation;
mod asm_nop_explanation;
mod naked_asm_explanation;
mod global_asm_explanation;
mod asm_inout_reg_explanation;
mod asm_lateout_reg_explanation;
mod asm_inlateout_reg_explanation;
mod asm_out_reg_explanation;
mod asm_in_reg_explanation;
mod asm_inout_split_explanation;
mod asm_const_operand_explanation;
mod asm_sym_operand_explanation;
mod asm_clobber_abi_explanation;
mod asm_readonly_explanation;
mod asm_preserves_flags_explanation;
mod asm_nomem_explanation;
mod asm_nostack_explanation;
mod asm_noreturn_explanation;
mod asm_att_syntax_explanation;
mod asm_label_operand_explanation;
mod asm_raw_option_explanation;
mod tuple_parameter_explanation;
mod wildcard_parameter_explanation;
mod asm_pure_nomem_explanation;
mod async_trait_requirement_explanation;
mod ref_parameter_explanation;
mod mutable_parameter_explanation;
mod gat_lifetime_declaration_explanation;
mod gat_self_outlives_explanation;
mod trait_default_const_explanation;
mod trait_self_return_requirement_explanation;
mod trait_opaque_iterator_explanation;
mod shared_reference_trait_impl_explanation;
mod handler_fn_alias_explanation;
mod pin_mut_self_receiver_explanation;
mod result_unit_error_return_explanation;
mod array_four_trait_impl_explanation;
mod mutable_reference_trait_impl_explanation;
mod try_result_option_explanation;
mod size_of_call_explanation;
mod std_drop_call_explanation;
mod align_of_call_explanation;
mod reserved_override_explanation;
mod reserved_abstract_explanation;
mod reserved_final_explanation;
mod reserved_do_explanation;
mod reserved_try_explanation;
mod reserved_typeof_explanation;
mod reserved_virtual_explanation;
mod reserved_yield_explanation;
mod reserved_box_explanation;
mod reserved_become_explanation;
mod reserved_macro_explanation;
mod reserved_unsized_explanation;
mod feature_gate_explanation;
mod reserved_gen_explanation;
mod reserved_priv_explanation;
mod negative_trait_impl_explanation;
mod trait_alias_unstable_explanation;
mod opaque_type_alias_explanation;
mod unit_struct_braced_explanation;
mod tuple_struct_braced_explanation;
mod tuple_struct_update_explanation;
mod tuple_swap_assignment_explanation;
mod unit_variant_braced_explanation;
mod array_discard_assignment_explanation;
mod single_tuple_binding_explanation;
mod struct_fields_assignment_explanation;
mod tuple_struct_assignment_explanation;
mod struct_ref_rest_binding_explanation;
mod match_range_to_inclusive_explanation;
mod legacy_range_pattern_explanation;
mod use_underscore_alias_explanation;
mod numeric_field_pattern_explanation;
mod leading_or_pattern_explanation;
mod empty_use_group_explanation;
mod nested_use_tree_explanation;
mod extern_crate_alias_explanation;
mod extern_self_alias_explanation;
mod should_panic_expected_explanation;
mod allow_dead_code_explanation;
mod deprecated_attribute_explanation;
mod warn_missing_docs_explanation;
mod expect_unused_variables_explanation;
mod macro_export_attribute_explanation;
mod automatically_derived_explanation;
mod deprecated_note_explanation;
mod macro_use_attribute_explanation;
mod forbid_unsafe_code_explanation;
mod deny_unused_variables_explanation;
mod one_hash_raw_string_explanation;
mod raw_string_no_hash_explanation;
mod octal_literal_explanation;
mod multiple_hash_raw_string_explanation;
mod raw_byte_string_explanation;
mod hex_integer_literal_explanation;
mod raw_byte_string_hash_explanation;
mod byte_string_literal_explanation;
mod byte_literal_explanation;
mod usize_literal_explanation;
mod positional_field_explanation;
mod half_open_for_range_explanation;
mod i32_suffixed_literal_explanation;
mod u8_literal_explanation;
mod single_tuple_type_explanation;
mod slice_type_explanation;
mod shared_reference_type_explanation;
mod shared_slice_reference_explanation;
mod mutable_reference_type_explanation;
mod mutable_slice_reference_explanation;
mod raw_const_pointer_explanation;
mod raw_mut_pointer_explanation;
mod dynamic_trait_type_explanation;
mod unsafe_function_pointer_explanation;
mod return_impl_trait_explanation;
mod impl_trait_parameter_explanation;
mod borrowed_dyn_trait_explanation;
mod function_pointer_type_explanation;
mod boxed_dyn_trait_explanation;
mod dyn_send_bound_explanation;
mod dyn_lifetime_explanation;
mod combined_impl_trait_explanation;
mod never_type_explanation;
mod named_shared_lifetime_explanation;
mod mutable_reference_lifetime_explanation;
mod function_lifetime_parameter_explanation;
mod static_lifetime_explanation;
mod inferred_type_placeholder_explanation;
mod inferred_lifetime_explanation;
mod type_outlives_explanation;
mod lifetime_outlives_explanation;
mod hrtb_trait_explanation;
mod generic_clone_bound_explanation;
mod named_generic_function_explanation;
mod for_lifetime_fn_pointer_explanation;
mod where_clause_explanation;
mod combined_generic_bounds_explanation;
mod where_type_requirement_explanation;
mod relaxed_sized_explanation;
mod where_associated_type_explanation;
mod sized_bound_explanation;
mod lifetime_send_bounds_explanation;
mod const_function_parameter_explanation;
mod default_type_parameter_explanation;
mod struct_const_parameter_explanation;
mod const_generic_default_explanation;
mod const_turbofish_explanation;
mod turbofish_type_explanation;
mod computed_const_generic_explanation;
mod type_qualified_call_explanation;
mod qualified_trait_call_explanation;
mod associated_type_equality_explanation;
mod qualified_assoc_type_explanation;
mod shared_borrow_expression_explanation;
mod mutable_borrow_expression_explanation;
mod precise_capture_explanation;
mod dereference_explanation;
mod raw_const_address_explanation;
mod raw_mut_address_explanation;
mod ref_pattern_explanation;
mod ref_mut_pattern_explanation;
mod mutable_pattern_binding_explanation;
mod block_expression_explanation;
mod semicolon_block_result_explanation;
mod if_else_value_explanation;
mod if_let_condition_explanation;
mod if_let_chain_explanation;
mod while_let_explanation;
mod for_iteration_explanation;
mod for_tuple_pattern_explanation;
mod loop_binding_result_explanation;
mod loop_label_explanation;
mod labeled_break_explanation;
mod labeled_continue_explanation;
mod labeled_break_value_explanation;
mod match_exhaustiveness_explanation;
mod match_arm_syntax_explanation;
mod match_guard_explanation;
mod const_fn_explanation;
mod unsafe_block_explanation;
mod unsafe_function_declaration_explanation;
mod async_block_explanation;
mod async_move_capture_explanation;
mod async_function_explanation;
mod await_postfix_explanation;
mod typed_closure_parameter_explanation;
mod zero_argument_closure_explanation;
mod closure_explicit_return_explanation;
mod basic_closure_explanation;
mod move_closure_explanation;
mod async_closure_explanation;
mod async_move_closure_explanation;
mod shared_fn_bound_explanation;
mod fnmut_bound_explanation;
mod fnonce_bound_explanation;
mod async_fn_once_bound_explanation;
mod async_fn_bound_explanation;
mod async_fn_mut_bound_explanation;
mod wildcard_pattern_explanation;
mod some_pattern_explanation;
mod ok_pattern_explanation;
mod none_pattern_explanation;
mod or_pattern_syntax_explanation;
mod err_pattern_explanation;
mod open_lower_range_pattern_explanation;
mod inclusive_match_range_explanation;
mod at_binding_pattern_explanation;
mod inclusive_range_binding_explanation;
mod reference_pattern_explanation;
mod at_pattern_binding_explanation;
mod mut_reference_pattern_explanation;
mod struct_pattern_rest_explanation;
mod some_ref_pattern_explanation;
mod named_field_range_pattern_explanation;
mod explicit_field_pattern_explanation;
mod named_field_match_pattern_explanation;
mod some_ref_mut_match_explanation;
mod tuple_rest_pattern_explanation;
mod named_positional_pattern_explanation;
mod tuple_pattern_explanation;
mod slice_endpoint_pattern_explanation;
mod exact_slice_pattern_explanation;
mod two_element_array_let_explanation;
mod half_open_range_explanation;
mod inclusive_range_expression_explanation;
mod range_to_expression_explanation;
mod inclusive_range_to_expression_explanation;
mod open_end_range_expression_explanation;
mod rest_dots_explanation;
mod half_open_range_index_explanation;
mod full_range_index_explanation;
mod open_ended_index_explanation;
mod inclusive_prefix_index_explanation;
mod open_start_slice_index_explanation;
mod match_middle_slice_pattern_explanation;
mod nested_tuple_variant_pattern_explanation;
mod range_to_pattern_explanation;
mod half_open_range_pattern_explanation;
mod inline_const_block_explanation;
mod question_mark_general_explanation;
mod labeled_block_value_explanation;
mod match_wildcard_arm_explanation;
mod let_block_value_explanation;
mod associated_type_bound_explanation;
mod value_binding_transfer_explanation;
mod method_turbofish_explanation;
mod turbofish_type_path_explanation;
mod integer_128_explanation;
mod signed_integer_explanation;
mod float_type_explanation;
mod char_type_explanation;
mod str_reference_explanation;
mod string_type_explanation;

use std::collections::HashSet;
use std::path::Path;

use nodex_ir::{Diagnostic, FileIr, ImportRef, Language, Severity};
use nodex_lang_api::{LanguageFrontend, ProjectDescriptor, SemanticProvider, SourceFile, TraceInstrumenter, kit::LineIndex};

pub use structure::{module_of, module_path};

/// The Rust language frontend.
#[derive(Debug, Default, Clone, Copy)]
pub struct RustFrontend;

impl LanguageFrontend for RustFrontend {
    fn id(&self) -> Language {
        Language::Rust
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn detect(&self, root: &Path) -> Option<ProjectDescriptor> {
        project::detect(root)
    }

    fn lower_file(&self, file: &SourceFile) -> FileIr {
        lower_source(&file.rel_path, &file.text)
    }

    fn semantic(&self, root: &Path) -> Option<Result<Box<dyn SemanticProvider>, String>> {
        Some(semantic::RustAnalyzer::start(root).map(|s| Box::new(s) as Box<dyn SemanticProvider>))
    }

    fn debugger(&self, _root: &Path) -> Option<Box<dyn nodex_lang_api::DebugLauncher>> {
        Some(Box::new(debug::RustDebugger))
    }

    fn instrumenter(&self, _root: &Path) -> Option<Box<dyn TraceInstrumenter>> {
        Some(Box::new(instrument::RustInstrumenter))
    }
}

/// The tree-sitter Rust grammar.
pub fn grammar() -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

/// Parses Rust text. When the grammar fails, same-length rewrites bridge syntax it predates
/// (ADR-011); a rewrite is kept only if it reduces the errors. Spans and labels keep reading
/// the original text.
pub fn parse(text: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&grammar()).ok()?;
    let tree = parser.parse(text, None)?;
    if !tree.root_node().has_error() {
        return Some(tree);
    }
    let mut best = (error_count(&tree), tree);
    let mut current = text.to_owned();
    for rule in [blank_safe_keyword as fn(&str) -> Option<String>, rename_try_macro, unicode_char_escape_explanation::repair_unicode_char_underscores] {
        let Some(next) = rule(&current) else { continue };
        debug_assert_eq!(next.len(), current.len());
        let Some(t) = parser.parse(&next, None) else { continue };
        let n = error_count(&t);
        if n < best.0 {
            best = (n, t);
            current = next;
        }
    }
    Some(best.1)
}

fn error_count(tree: &tree_sitter::Tree) -> usize {
    let mut count = 0;
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if n.is_error() || n.is_missing() {
            count += 1;
            continue;
        }
        if n.has_error() {
            let mut c = n.walk();
            stack.extend(n.children(&mut c));
        }
    }
    count
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Byte offsets of `word` as a whole word.
fn word_at(text: &str, word: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    text.match_indices(word)
        .map(|(i, _)| i)
        .filter(|&i| (i == 0 || !is_ident(bytes[i - 1])) && bytes.get(i + word.len()).is_none_or(|&b| !is_ident(b)))
        .collect()
}

/// Rust 2024 `unsafe extern { safe fn f(); }`: the grammar does not know `safe`.
fn blank_safe_keyword(text: &str) -> Option<String> {
    let hits: Vec<usize> = word_at(text, "safe").into_iter().filter(|&i| text[i + 4..].trim_start().starts_with("fn") || text[i + 4..].trim_start().starts_with("static")).collect();
    if hits.is_empty() {
        return None;
    }
    let mut out = text.to_owned();
    for i in hits {
        out.replace_range(i..i + 4, "    ");
    }
    Some(out)
}

/// `try!(x)` (Rust 2015): `try` is a keyword since 2018, so the grammar rejects the macro.
fn rename_try_macro(text: &str) -> Option<String> {
    let hits: Vec<usize> = word_at(text, "try").into_iter().filter(|&i| text[i + 3..].starts_with('!')).collect();
    if hits.is_empty() {
        return None;
    }
    let mut out = text.to_owned();
    for i in hits {
        out.replace_range(i..i + 3, "Try");
    }
    Some(out)
}

/// Where the grammar fails: `(line, first line of the failing code)`. A diagnostic aid for
/// corpus testing.
pub fn remaining_parse_errors(text: &str) -> Vec<(usize, String)> {
    let text = without_bom(text);
    let Some(tree) = parse(&text) else { return Vec::new() };
    let mut out = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        let mut c = n.walk();
        let failing: Vec<_> = n.children(&mut c).filter(|k| k.is_error() || k.is_missing() || k.has_error()).collect();
        if (n.is_error() || n.is_missing()) && failing.is_empty() {
            let first = text.get(n.byte_range()).and_then(|t| t.lines().find(|l| !l.trim().is_empty())).unwrap_or("").trim();
            out.push((n.start_position().row + 1, first.chars().take(80).collect::<String>()));
            continue;
        }
        stack.extend(failing);
    }
    out.sort();
    out
}

/// The text with a leading byte order mark replaced by spaces of the same byte length (spans
/// keep matching the file).
fn without_bom(text: &str) -> String {
    match text.strip_prefix('\u{feff}') {
        Some(rest) => format!("   {rest}"),
        None => text.to_owned(),
    }
}

/// Lowers Rust source. Never fails: parse problems become diagnostics and `Opaque` nodes.
pub fn lower_source(rel_path: &str, text: &str) -> FileIr {
    let mut ir = FileIr {
        borrow_models: Default::default(),
        source_revision: None,
        file: rel_path.to_owned(),
        language: Language::Rust,
        symbols: Vec::new(),
        flows: Vec::new(),
        imports: Vec::new(),
        type_refs: Vec::new(),
        diagnostics: Vec::new(),
        knowledge: Default::default(),
    };
    let prepared = without_bom(text);
    let Some(tree) = parse(&prepared) else {
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: "tree-sitter Rust grammar is incompatible with the runtime".into(),
            span: None,
        });
        return ir;
    };
    let index = LineIndex::new(text);
    let root = tree.root_node();
    if root.has_error() {
        let first_error = syntax::first_error(root);
        ir.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "The file has syntax errors; the affected code appears as Opaque nodes.".into(),
            span: first_error.map(|n| index.span(rel_path, n.start_byte(), n.end_byte())),
        });
    }
    let src = syntax::Src { text, file: rel_path, index: &index };
    let mut collector = structure::Collector::new(src);
    collector.source_file(root);
    let structure::Collected { mut symbols, imports, mut type_refs, bodies, function_ids, module } = collector.finish();

    let copy_types: HashSet<String> = symbols
        .iter()
        .filter(|s| s.flavors.iter().any(|f| f == "derive:Copy"))
        .map(|s| s.name.clone())
        .collect();
    // Types of this file with `impl Drop`: their locals run `drop` when their scope ends.
    let drop_types: std::collections::HashMap<String, nodex_ir::SymbolId> = symbols
        .iter()
        .filter(|s| s.name == "drop" && s.flavors.iter().any(|f| f == "from-trait:Drop"))
        .filter_map(|s| {
            let owner = s.parent.as_ref()?;
            let ty = symbols.iter().find(|t| &t.id == owner)?;
            Some((ty.name.clone(), s.id.clone()))
        })
        .collect();
    for job in &bodies {
        let lowered = flow::lower_body(src, job, &function_ids, &copy_types, &drop_types);
        type_refs.extend(lowered.type_refs);
        if let Some(sym) = symbols.iter_mut().find(|s| s.id == job.symbol) {
            sym.has_flow = true;
        }
        ir.flows.push(lowered.graph);
    }
    ir.knowledge = knowledge::extract(src, root, &symbols);
    ir.imports = imports.into_iter().map(|target| ImportRef { from_scope: module.clone(), target }).collect();
    ir.symbols = symbols;
    ir.type_refs = type_refs;
    nodex_lang_api::syntax_sites::annotate(&mut ir, root, src, false);
    nodex_lang_api::value_shapes::annotate(&mut ir, root, src);
    nodex_lang_api::blueprint_types::annotate(&mut ir, root, src, &Default::default());
    let (reading, _, _) = reading::collect(src, root, &Default::default());
    for (job, graph) in bodies.iter().zip(&mut ir.flows) {
        graph.attach_reading(&reading);
        let model = borrow::analyze(src, job, graph);
        if !model.uses.is_empty() {
            borrow::annotate_control_values(graph, &model);
            ir.borrow_models.insert(graph.symbol.clone(), model);
        }
    }
    ir
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_ir::{Concept, SymbolKind};

    #[test]
    fn lowers_a_small_crate_file() {
        let text = r#"
//! Orders.
use std::collections::HashMap;

/// An order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Order { pub id: u32, pub amount: i64 }

pub trait Priced { fn price(&self) -> i64; }

impl Priced for Order {
    fn price(&self) -> i64 { self.amount }
}

pub fn sum_of_even_squares(numbers: &[i32]) -> i32 {
    let mut total = 0;
    for n in numbers {
        if n % 2 == 0 {
            total += n * n;
        }
    }
    total
}
"#;
        let ir = lower_source("src/orders.rs", text);
        assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
        let order = ir.symbols.iter().find(|s| s.id == "crate::orders::Order").expect("struct");
        assert_eq!(order.kind, SymbolKind::Struct);
        assert!(order.flavors.iter().any(|f| f == "derive:Copy"));
        assert_eq!(order.doc.as_deref(), Some("An order."));
        assert!(ir.symbols.iter().any(|s| s.id == "crate::orders::Order::price" && s.kind == SymbolKind::Method));
        assert!(ir.symbols.iter().any(|s| s.id == "crate::orders::Order::amount"));
        assert!(ir.imports.iter().any(|i| i.target == "pkg:std"));
        let flow = ir.flows.iter().find(|f| f.symbol == "crate::orders::sum_of_even_squares").expect("flow");
        let concepts: Vec<Concept> = flow.nodes.iter().map(|n| n.concept).collect();
        assert_eq!(concepts, [Concept::Entry, Concept::Var, Concept::LoopForEach, Concept::Branch, Concept::Assign, Concept::Return]);
    }

    #[test]
    fn byte_order_mark_does_not_break_parsing() {
        let ir = lower_source("src/lib.rs", "\u{feff}pub fn f() -> i32 { 1 }\n");
        assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
        assert_eq!(ir.flows.len(), 1);
    }
}

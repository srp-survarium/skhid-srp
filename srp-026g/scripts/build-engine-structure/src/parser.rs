pub fn parse_function_name(input: &str) -> String {
    let mut func_sig = "";
    for line in input.lines() {
        if !line.starts_with("//") {
            func_sig = line;
            break;
        }
    }

    let mut input = func_sig
        .chars()
        .take_while(|&c| c != '\n')
        .collect::<String>();

    // Remove operator sign from all operators.
    // All of them will be stored in a single file.
    // This is done before removing generics because `operator<` exists
    if let Some(idx) = input.find("::operator") {
        let new_len = idx + "::operator".len();
        if let Some(&c) = input.as_bytes().get(new_len)
            && !(c.is_ascii_alphanumeric() || c == b'_')
        {
            input.truncate(new_len);
        }
    }

    // Remove generics.
    let mut depth = 0;
    let input = input
        .chars()
        .filter(|&c| match c {
            '<' => {
                depth += 1;
                false
            }
            '>' => {
                depth -= 1;
                false
            }
            _ if depth == 0 => true,
            _ => false,
        })
        .collect::<String>();

    // Ugly way to deal with functions return functions.
    let (_, input) = input.split_once(" ").unwrap();
    let input = input.trim_start_matches('(');

    let mut in_quotes = false;
    let input = input
        .chars()
        // Fix up `vector destructor iterator'
        .filter_map(|c| match c {
            '`' => {
                in_quotes = true;
                None
            }
            '\'' => {
                in_quotes = true;
                None
            }
            ' ' if in_quotes => Some('_'),
            c => Some(c),
        })
        // Ignore arguments
        .take_while(|&c| c != '(')
        .collect::<String>();

    let func_name = input
        .split(' ')
        // Remove calling convention specification
        .next_back()
        .unwrap();
    // Remove register return type for custom calling conventions
    let func_name = func_name.strip_suffix('@').unwrap_or(func_name);
    let func_name = func_name.trim_start_matches('*').trim_start_matches('*');
    let result = func_name.to_string();

    match result.is_empty() {
        true => "fail".to_string(),
        false => result,
    }
}

#[test]
fn function_names() {
    let func_sig = "vostok::animation::mixing::expression *__thiscall survarium::weapon_core_idle_state::weapon_and_hands_expression(";
    assert_eq!(
        "survarium::weapon_core_idle_state::weapon_and_hands_expression",
        parse_function_name(func_sig),
    );

    let func_sig = "void __thiscall vostok::ai::planning::operator_impl::execute";
    assert_eq!(
        "vostok::ai::planning::operator_impl::execute",
        parse_function_name(func_sig),
    );

    let func_sig = "stlp_std::priv::_Rb_tree_iterator<stlp_std::pair<unsigned int const,survarium::base_point_stats,>,stlp_std::priv::_MapTraitsT<stlp_std::pair<unsigned int const,survarium::base_point_stats>>>* __usercall stlp_std::map<unsigned int,survarium::base_point_stats,stlp_std::less<unsigned int>,survarium::std_allocator<stlp_std::pair<unsigned int,survarium::base_point_stats,>>>::operator[]<unsigned int>@<eax>(";
    assert_eq!("stlp_std::map::operator", parse_function_name(func_sig));

    let func_sig = "unsigned __int8 __usercall vostok::configs::binary_config_value::operator unsigned char@<al>(";
    assert_eq!(
        "vostok::configs::binary_config_value::operator",
        parse_function_name(func_sig)
    );

    let func_sig = "void (__thiscall *Value_const___::Method__())(Scaleform::GFx::AS3::Instances::fl::Date *this, Scaleform::GFx::AS3::Value *result, unsigned int argc, Scaleform::GFx::AS3::Value *argv)";
    assert_eq!("Value_const___::Method__", parse_function_name(func_sig));

    let func_sig = "int __cdecl sub_541C80(int (__cdecl **a1)(int, int, int, int, int), int a2, int a3, int a4, int a5)";
    assert_eq!("sub_541C80", parse_function_name(func_sig));

    let func_sig = "void __stdcall `vector destructor iterator'(char *__t, unsigned int __s, int __n, void (__thiscall *__f)(void";
    assert_eq!("vector_destructor_iterator", parse_function_name(func_sig));

    let func_sig = "bool __thiscall Scaleform::GFx::ASString::operator<(";
    assert_eq!(
        "Scaleform::GFx::ASString::operator",
        parse_function_name(func_sig)
    );

    let func_sig = "void __cdecl DES_set_odd_parity(unsigned __int8 (*key)[8])";
    assert_eq!("DES_set_odd_parity", parse_function_name(func_sig));
}

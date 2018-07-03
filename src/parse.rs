
const AND_OPERATOR_STR : &str = "AND";
const OR_OPERATOR_STR : &str = "OR";

#[derive(Debug, Clone, PartialEq)]
pub enum Operator { AND, OR }
use self::Operator::*;
impl Operator {
    fn compare(&self, other : &Operator) -> i8 {
        match (self, other) {
            (&AND, &OR) => 1,
            (&OR, &AND) => -1,
            _ => 0
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Operand(String),
    Operator(Operator)
}

fn str_to_operator(op_str : &str) -> Option<Operator> {
    if op_str == AND_OPERATOR_STR {
        Some(AND)
    }
    else if op_str == OR_OPERATOR_STR {
        Some(OR)
    }
    else {
        None
    }
}

pub fn infix_to_postfix(infix : String) -> Vec<Arg> {
    let infix : Vec<&str> = infix.split(' ').collect();
    let mut stack = Vec::new();
    let mut postfix : Vec<Arg> = Vec::new();
    for arg in infix {
        if arg == AND_OPERATOR_STR || arg == OR_OPERATOR_STR {
            let arg = str_to_operator(arg).unwrap();
            if stack.is_empty() {
                stack.push(arg);
            }
            else {
                while !stack.is_empty() {
                    let mut top_stack = stack.get(stack.len() - 1).unwrap().clone();
                    let mut compare = arg.compare(&top_stack);
                    if compare > 0 {
                        break;
                    }
                    else {
                        postfix.push(Arg::Operator(stack.pop().unwrap()));
                    }
                }
                stack.push(arg);
            }
        }
        else {
            postfix.push(Arg::Operand(arg.to_string()));
        }
    }
    for op in stack.into_iter().rev() {
        postfix.push(Arg::Operator(op));
    }
    postfix
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_infix_to_postfix_1() {
        let infix = String::from("bob AND fred");
        let postfix = vec![
            Arg::Operand(String::from("bob")),
            Arg::Operand(String::from("fred")),
            Arg::Operator(Operator::AND)
        ];
        assert_eq!(infix_to_postfix(infix), postfix);
    }

    #[test]
    fn test_infix_to_postfix_2() {
        let infix = String::from("bob OR fred");
        let postfix = vec![
            Arg::Operand(String::from("bob")),
            Arg::Operand(String::from("fred")),
            Arg::Operator(Operator::OR)
        ];
        assert_eq!(infix_to_postfix(infix), postfix);
    }

    #[test]
    fn test_infix_to_postfix_3() {
        let infix = String::from("bob AND fred OR max");
        let postfix = vec![
            Arg::Operand(String::from("bob")),
            Arg::Operand(String::from("fred")),
            Arg::Operator(Operator::AND),
            Arg::Operand(String::from("max")),
            Arg::Operator(Operator::OR)
        ];
        assert_eq!(infix_to_postfix(infix), postfix);
    }

    #[test]
    fn test_infix_to_postfix_4() {
        let infix = String::from("bob OR fred AND max");
        let postfix = vec![
            Arg::Operand(String::from("bob")),
            Arg::Operand(String::from("fred")),
            Arg::Operand(String::from("max")),
            Arg::Operator(Operator::AND),
            Arg::Operator(Operator::OR)
        ];
        assert_eq!(infix_to_postfix(infix), postfix);
    }

    #[test]
    fn test_infix_to_postfix_5() {
        let infix = String::from("bob AND fred AND max");
        let postfix = vec![
            Arg::Operand(String::from("bob")),
            Arg::Operand(String::from("fred")),
            Arg::Operator(Operator::AND),
            Arg::Operand(String::from("max")),
            Arg::Operator(Operator::AND)
        ];
        assert_eq!(infix_to_postfix(infix), postfix);
    }

    #[test]
    fn test_infix_to_postfix_6() {
        let infix = String::from("bob AND fred OR max AND paul");
        let postfix = vec![
            Arg::Operand(String::from("bob")),
            Arg::Operand(String::from("fred")),
            Arg::Operator(Operator::AND),
            Arg::Operand(String::from("max")),
            Arg::Operand(String::from("paul")),
            Arg::Operator(Operator::AND),
            Arg::Operator(Operator::OR)
        ];
        assert_eq!(infix_to_postfix(infix), postfix);
    }
}

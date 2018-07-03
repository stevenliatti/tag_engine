
const AND_OPERATOR_STR : &str = "AND";
const OR_OPERATOR_STR : &str = "OR";

#[derive(Debug, Clone)]
enum Operator { AND, OR }

use self::Operator::*;
impl Operator {
    fn compare(&self, other : &Operator) -> i8 {
        match (self, other) {
            (&AND, &OR) => 1,
            (&OR, &AND) => -1,
            _ => 0
        }
    }

    fn str_to_operator(op_str : &str) -> Option<Self> {
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

    fn operator_to_str(op : &Operator) -> String {
        match op {
            &AND => AND_OPERATOR_STR.to_string(),
            &OR => OR_OPERATOR_STR.to_string()
        }
    }
}

pub fn infix_to_postfix(infix : String) -> Vec<String> {
    let infix : Vec<&str> = infix.split(' ').collect();
    let mut stack = Vec::new();
    let mut postfix = Vec::new();
    for arg in infix {
        if arg == AND_OPERATOR_STR || arg == OR_OPERATOR_STR {
            let arg = Operator::str_to_operator(arg).unwrap();
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
                        postfix.push(Operator::operator_to_str(&stack.pop().unwrap()));
                    }
                }
                stack.push(arg);
            }
        }
        else {
            postfix.push(arg.to_string());
        }
    }
    postfix.into_iter().map(|e| e.to_string()).collect()
}

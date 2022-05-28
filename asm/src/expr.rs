use crate::{
    intern::StrRef,
    symtab::{Symbol, Symtab},
};

#[derive(Copy, Clone, Debug)]
pub enum ExprNode {
    Value(i32),
    Label(StrRef),
    Invert,
    Not,
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    ShiftLeft,
    ShiftRight,
    ShiftLeftLogical,
    ShiftRightLogical,
    And,
    Or,
    Xor,
    AndLogical,
    OrLogical,
    LessThan,
    LessThanEqual,
    GreaterThan,
    GreaterThanEqual,
    Equal,
    NotEqual,
    Ternary,
}

#[derive(Clone, Debug)]
pub struct Expr {
    nodes: Vec<ExprNode>,
}

impl Expr {
    #[inline]
    pub fn new(nodes: Vec<ExprNode>) -> Self {
        Self { nodes }
    }

    #[inline]
    pub fn evaluate(&self, symtab: &Symtab) -> Option<i32> {
        let mut stack = Vec::new();
        for &node in &self.nodes {
            match node {
                ExprNode::Value(value) => stack.push(value),
                ExprNode::Label(strref) => match symtab.get(strref)? {
                    Symbol::Value(value) => stack.push(*value),
                    Symbol::Expr(expr) => stack.push(expr.evaluate(symtab)?),
                },
                ExprNode::Invert => {
                    let value = stack.pop().unwrap();
                    stack.push(!value);
                }
                ExprNode::Not => {
                    let value = stack.pop().unwrap();
                    stack.push((value == 0) as i32);
                }
                ExprNode::Neg => {
                    let value = stack.pop().unwrap();
                    stack.push(-value);
                }
                ExprNode::Add => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(lhs.wrapping_add(rhs));
                }
                ExprNode::Sub => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(lhs.wrapping_sub(rhs));
                }
                ExprNode::Mul => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(lhs.wrapping_mul(rhs));
                }
                ExprNode::Div => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(lhs.wrapping_div(rhs));
                }
                ExprNode::Mod => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(lhs.wrapping_rem(rhs));
                }
                ExprNode::ShiftLeft => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(lhs.wrapping_shl(rhs as u32));
                }
                ExprNode::ShiftRight => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(lhs.wrapping_shr(rhs as u32));
                }
                ExprNode::ShiftLeftLogical => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push((lhs as u32).wrapping_shl(rhs as u32) as i32);
                }
                ExprNode::ShiftRightLogical => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push((lhs as u32).wrapping_shr(rhs as u32) as i32);
                }
                ExprNode::And => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(lhs & rhs);
                }
                ExprNode::Or => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(lhs | rhs);
                }
                ExprNode::Xor => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(lhs & rhs);
                }
                ExprNode::AndLogical => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(((lhs != 0) && (rhs != 0)) as i32);
                }
                ExprNode::OrLogical => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(((lhs != 0) || (rhs != 0)) as i32);
                }
                ExprNode::LessThan => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push((lhs < rhs) as i32);
                }
                ExprNode::LessThanEqual => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push((lhs <= rhs) as i32);
                }
                ExprNode::GreaterThan => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push((lhs > rhs) as i32);
                }
                ExprNode::GreaterThanEqual => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push((lhs >= rhs) as i32);
                }
                ExprNode::Equal => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push((lhs == rhs) as i32);
                }
                ExprNode::NotEqual => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push((lhs != rhs) as i32);
                }
                ExprNode::Ternary => {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    let cond = stack.pop().unwrap();
                    stack.push(if cond != 0 { lhs } else { rhs });
                }
            }
        }
        stack.pop()
    }
}

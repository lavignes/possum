use crate::{
    intern::StrRef,
    symtab::{Symbol, Symtab},
};

#[derive(Clone, Debug)]
pub struct Expr {
    nodes: Vec<ExprNode>,
}

impl Expr {
    #[inline]
    pub fn evaluate(&self, symtab: &Symtab) -> Option<i32> {
        Self::eval(symtab, &self.nodes, 0)
    }

    #[inline]
    pub fn value(value: i32) -> Self {
        Self {
            nodes: vec![ExprNode::Value(value)],
        }
    }

    fn eval(symtab: &Symtab, nodes: &Vec<ExprNode>, index: usize) -> Option<i32> {
        match nodes[index] {
            ExprNode::Value(value) => Some(value),
            ExprNode::Label(label) => match symtab.get(label) {
                Some(Symbol::Expr(_)) => None,
                Some(Symbol::Value(value)) => Some(*value as i32),
                _ => None,
            },
            ExprNode::Invert(index) => Self::eval(symtab, nodes, index).map(|value| !value),
            ExprNode::Not(index) => {
                Self::eval(symtab, nodes, index).map(|value| (value != 0) as i32)
            }
            ExprNode::Neg(index) => Self::eval(symtab, nodes, index).map(|value| -value),
            ExprNode::Add(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(lhs.wrapping_add(rhs))
            }
            ExprNode::Sub(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(lhs.wrapping_sub(rhs))
            }
            ExprNode::Mul(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(lhs.wrapping_mul(rhs))
            }
            ExprNode::Div(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(lhs.wrapping_div(rhs))
            }
            ExprNode::Mod(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(lhs.wrapping_rem(rhs))
            }
            ExprNode::ShiftLeft(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(lhs.wrapping_shl(rhs as u32))
            }
            ExprNode::ShiftRight(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(lhs.wrapping_shr(rhs as u32))
            }
            ExprNode::And(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(lhs & rhs)
            }
            ExprNode::Or(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(lhs | rhs)
            }
            ExprNode::Xor(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(lhs ^ rhs)
            }
            ExprNode::AndLogical(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(((lhs != 0) && (rhs != 0)) as i32)
            }
            ExprNode::OrLogical(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(((lhs != 0) || (rhs != 0)) as i32)
            }
            ExprNode::LessThan(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some((lhs < rhs) as i32)
            }
            ExprNode::LessThanEqual(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some((lhs <= rhs) as i32)
            }
            ExprNode::GreaterThan(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some((lhs > rhs) as i32)
            }
            ExprNode::GreaterThanEqual(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some((lhs >= rhs) as i32)
            }
            ExprNode::Equal(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some((lhs == rhs) as i32)
            }
            ExprNode::NotEquals(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some((lhs != rhs) as i32)
            }
            ExprNode::Ternary(condition, lhs, rhs) => {
                let condition = Self::eval(symtab, nodes, condition)?;
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                Some(if condition != 0 { lhs } else { rhs })
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ExprNode {
    Value(i32),
    Label(StrRef),
    Invert(usize),
    Not(usize),
    Neg(usize),
    Add(usize, usize),
    Sub(usize, usize),
    Mul(usize, usize),
    Div(usize, usize),
    Mod(usize, usize),
    ShiftLeft(usize, usize),
    ShiftRight(usize, usize),
    And(usize, usize),
    Or(usize, usize),
    Xor(usize, usize),
    AndLogical(usize, usize),
    OrLogical(usize, usize),
    LessThan(usize, usize),
    LessThanEqual(usize, usize),
    GreaterThan(usize, usize),
    GreaterThanEqual(usize, usize),
    Equal(usize, usize),
    NotEquals(usize, usize),
    Ternary(usize, usize, usize),
}

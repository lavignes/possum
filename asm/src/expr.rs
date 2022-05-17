use crate::{
    intern::StrRef,
    symtab::{Symbol, Symtab},
};

#[derive(thiserror::Error, Debug)]
pub enum ExprError {}

#[derive(Clone, Debug)]
pub struct Expr {
    nodes: Vec<ExprNode>,
}

impl Expr {
    #[inline]
    pub fn evaluate(&self, symtab: &Symtab) -> Result<Option<isize>, ExprError> {
        Self::eval(symtab, &self.nodes, 0)
    }

    fn eval(
        symtab: &Symtab,
        nodes: &Vec<ExprNode>,
        index: usize,
    ) -> Result<Option<isize>, ExprError> {
        match nodes[index] {
            ExprNode::Value(value) => Ok(Some(value)),
            ExprNode::Label(label) => match symtab.get(label) {
                Some(Symbol::Expr(_)) => Ok(None),
                Some(Symbol::Value(value)) => Ok(Some(*value)),
                _ => Ok(None),
            },
            ExprNode::Invert(index) => match Self::eval(symtab, nodes, index)? {
                Some(value) => Ok(Some(!value)),
                _ => Ok(None),
            },
            ExprNode::Not(index) => match Self::eval(symtab, nodes, index)? {
                Some(value) => Ok(Some(!(value != 0) as isize)),
                _ => Ok(None),
            },
            ExprNode::Neg(index) => match Self::eval(symtab, nodes, index)? {
                Some(value) => Ok(Some(-value)),
                _ => Ok(None),
            },
            ExprNode::Add(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(lhs.wrapping_add(rhs))),
                    _ => Ok(None),
                }
            }
            ExprNode::Sub(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(lhs.wrapping_sub(rhs))),
                    _ => Ok(None),
                }
            }
            ExprNode::Mul(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(lhs.wrapping_mul(rhs))),
                    _ => Ok(None),
                }
            }
            ExprNode::Div(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(lhs.wrapping_div(rhs))),
                    _ => Ok(None),
                }
            }
            ExprNode::Mod(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(lhs.wrapping_rem(rhs))),
                    _ => Ok(None),
                }
            }
            ExprNode::ShiftLeft(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(lhs.wrapping_shl(rhs as u32))),
                    _ => Ok(None),
                }
            }
            ExprNode::ShiftRight(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(lhs.wrapping_shr(rhs as u32))),
                    _ => Ok(None),
                }
            }
            ExprNode::And(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(lhs & rhs)),
                    _ => Ok(None),
                }
            }
            ExprNode::Or(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(lhs | rhs)),
                    _ => Ok(None),
                }
            }
            ExprNode::Xor(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(lhs ^ rhs)),
                    _ => Ok(None),
                }
            }
            ExprNode::AndLogical(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(((lhs != 0) && (rhs != 0)) as isize)),
                    _ => Ok(None),
                }
            }
            ExprNode::OrLogical(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some(((lhs != 0) || (rhs != 0)) as isize)),
                    _ => Ok(None),
                }
            }
            ExprNode::LessThan(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some((lhs < rhs) as isize)),
                    _ => Ok(None),
                }
            }
            ExprNode::LessThanEqual(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some((lhs <= rhs) as isize)),
                    _ => Ok(None),
                }
            }
            ExprNode::GreaterThan(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some((lhs > rhs) as isize)),
                    _ => Ok(None),
                }
            }
            ExprNode::GreaterThanEqual(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some((lhs >= rhs) as isize)),
                    _ => Ok(None),
                }
            }
            ExprNode::Equal(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some((lhs == rhs) as isize)),
                    _ => Ok(None),
                }
            }
            ExprNode::NotEquals(lhs, rhs) => {
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (lhs, rhs) {
                    (Some(lhs), Some(rhs)) => Ok(Some((lhs != rhs) as isize)),
                    _ => Ok(None),
                }
            }
            ExprNode::Ternary(condition, lhs, rhs) => {
                let condition = Self::eval(symtab, nodes, condition)?;
                let lhs = Self::eval(symtab, nodes, lhs)?;
                let rhs = Self::eval(symtab, nodes, rhs)?;
                match (condition, lhs, rhs) {
                    (Some(condition), Some(lhs), Some(rhs)) => {
                        Ok(Some(if condition != 0 { lhs } else { rhs }))
                    }
                    _ => Ok(None),
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ExprNode {
    Value(isize),
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

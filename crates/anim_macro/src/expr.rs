#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    Var(String),
    Unary(char, Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

pub struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    pub fn parse(input: &str) -> Result<Expr, String> {
        let mut p = Self {
            chars: input.chars().collect(),
            pos: 0,
        };
        let expr = p.expr()?;
        p.skip_ws();
        if p.pos != p.chars.len() {
            return Err(format!("unexpected trailing input at char {}", p.pos));
        }
        Ok(expr)
    }

    fn expr(&mut self) -> Result<Expr, String> {
        let mut lhs = self.term()?;
        loop {
            self.skip_ws();
            let Some(op) = self.peek_char().filter(|c| *c == '+' || *c == '-') else {
                break;
            };
            self.pos += 1;
            let rhs = self.term()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut lhs = self.unary()?;
        loop {
            self.skip_ws();
            let Some(op) = self
                .peek_char()
                .filter(|c| *c == '*' || *c == '/' || *c == '%')
            else {
                break;
            };
            self.pos += 1;
            let rhs = self.unary()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        self.skip_ws();
        if let Some(op) = self.peek_char().filter(|c| *c == '+' || *c == '-') {
            self.pos += 1;
            return Ok(Expr::Unary(op, Box::new(self.unary()?)));
        }
        self.power()
    }

    fn power(&mut self) -> Result<Expr, String> {
        let base = self.primary()?;
        self.skip_ws();
        if self.peek_char() == Some('^') {
            self.pos += 1;
            let exponent = self.unary()?;
            return Ok(Expr::Binary('^', Box::new(base), Box::new(exponent)));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        self.skip_ws();
        if self.peek_char() == Some('(') {
            self.pos += 1;
            let inner = self.expr()?;
            self.expect(')')?;
            return Ok(inner);
        }
        if self
            .peek_char()
            .is_some_and(|c| c.is_ascii_digit() || c == '.')
        {
            return self.number();
        }
        if self
            .peek_char()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        {
            let ident = self.ident();
            self.skip_ws();
            if self.peek_char() == Some('(') {
                self.pos += 1;
                let mut args = Vec::new();
                if self.peek_char() != Some(')') {
                    loop {
                        args.push(self.expr()?);
                        self.skip_ws();
                        if self.peek_char() == Some(',') {
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                }
                self.expect(')')?;
                return Ok(Expr::Call(ident, args));
            }
            return Ok(Expr::Var(ident));
        }
        Err(format!("expected expression at char {}", self.pos))
    }

    fn number(&mut self) -> Result<Expr, String> {
        let start = self.pos;
        while self
            .peek_char()
            .is_some_and(|c| c.is_ascii_digit() || c == '.')
        {
            self.pos += 1;
        }
        if self.peek_char().is_some_and(|c| c == 'e' || c == 'E') {
            self.pos += 1;
            if self.peek_char().is_some_and(|c| c == '+' || c == '-') {
                self.pos += 1;
            }
            while self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let raw: String = self.chars[start..self.pos].iter().collect();
        raw.parse::<f64>()
            .map(Expr::Num)
            .map_err(|_| format!("bad number `{raw}`"))
    }

    fn ident(&mut self) -> String {
        let start = self.pos;
        while self
            .peek_char()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            self.pos += 1;
        }
        self.chars[start..self.pos].iter().collect()
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        self.skip_ws();
        if self.peek_char() == Some(expected) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected `{expected}` at char {}", self.pos))
        }
    }

    fn skip_ws(&mut self) {
        while self.peek_char().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::Parser;

    #[test]
    fn parses_precedence() {
        assert!(Parser::parse("sin(x) + y * 2^3").is_ok());
    }

    #[test]
    fn rejects_garbage() {
        assert!(Parser::parse("sin(x) + @").is_err());
    }
}

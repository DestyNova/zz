use pest::Parser;
use super::ast::*;
use super::name::Name;
use std::path::Path;
use std::io::{Read};
use super::pp::PP;

#[derive(Parser)]
#[grammar = "zz.pest"]
pub struct ZZParser;


pub fn make_error<S: Into<String>>(loc: &Location, message: S) -> pest::error::Error<Rule> {
    pest::error::Error::<Rule>::new_from_span(pest::error::ErrorVariant::CustomError {
        message: message.into(),
    }, loc.span.clone()).with_path(&loc.file)
}

pub fn parse(n: &Path) -> Module
{
    match p(&n){
        Err(e) => {
            let e = e.with_path(&n.to_string_lossy());
            error!("syntax error\n{}", e);
            std::process::exit(9);
        }
        Ok(md) => {
            md
        }
    }
}

fn p(n: &Path) -> Result<Module, pest::error::Error<Rule>> {

    let mut module = Module::default();
    module.source = n.to_path_buf();
    module.sources.insert(n.canonicalize().unwrap());
    module.name.push(n.file_stem().expect(&format!("stem {:?}", n)).to_string_lossy().into());

    let mut f = std::fs::File::open(n).expect(&format!("cannot open file {:?}", n));
    let mut file_str = String::new();
    f.read_to_string(&mut file_str).expect(&format!("read {:?}", n));
    let file_str = Box::leak(Box::new(file_str));
    let mut file = ZZParser::parse(Rule::file, file_str)?;


    for decl in PP::new(n, file.next().unwrap().into_inner()) {
        match decl.as_rule() {
            Rule::imacro => {
                let loc = Location{
                    file: n.to_string_lossy().into(),
                    span: decl.as_span(),
                };
                let decl = decl.into_inner();
                let mut name = None;
                let mut args = Vec::new();
                let mut export_as = None;
                let mut body = None;
                let mut vis = Visibility::Object;
                for part in decl {
                    match part.as_rule() {
                        Rule::key_shared => {
                            vis = Visibility::Shared;
                        }
                        Rule::exported => {
                            vis = Visibility::Export;
                            for part in part.into_inner() {
                                match part.as_rule() {
                                    Rule::ident => {
                                        export_as = Some(part.as_str().to_string());
                                    },
                                    e => panic!("unexpected rule {:?} in export", e),
                                }
                            }
                        }
                        Rule::ident if name.is_none() => {
                            name = part.as_str().into();
                        }
                        Rule::macro_args => {
                            for arg in part.into_inner() {
                                args.push(arg.as_str().into());
                            }
                        }
                        Rule::block if body.is_none() => {
                            body = Some(parse_block((file_str, n), part));
                        },
                        e => panic!("unexpected rule {:?} in macro ", e),
                    }
                }

                module.locals.push(Local{
                    export_as,
                    name: name.unwrap().to_string(),
                    vis,
                    loc,
                    def:  Def::Macro{
                        args,
                        body: body.unwrap(),
                    }
                });

            }
            Rule::function => {
                let loc = Location{
                    file: n.to_string_lossy().into(),
                    span: decl.as_span(),
                };
                let decl = decl.into_inner();
                let mut name = String::new();
                let mut export_as = None;
                let mut args = Vec::new();
                let mut ret  = None;
                let mut body = None;
                let mut vararg = false;
                let mut vis = Visibility::Object;

                for part in decl {
                    match part.as_rule() {
                        Rule::key_shared => {
                            vis = Visibility::Shared;
                        }
                        Rule::exported => {
                            vis = Visibility::Export;
                            for part in part.into_inner() {
                                match part.as_rule() {
                                    Rule::ident => {
                                        export_as = Some(part.as_str().to_string());
                                    },
                                    e => panic!("unexpected rule {:?} in export", e),
                                }
                            }
                        }
                        Rule::ident => {
                            name = part.as_str().into();
                        }
                        Rule::ret_arg => {
                            let part = part.into_inner().next().unwrap();
                            ret = Some(AnonArg{
                                typed: parse_anon_type((file_str, n), part),
                            });
                        },
                        Rule::fn_args => {
                            for arg in part.into_inner() {

                                let argloc  = Location{
                                    file: n.to_string_lossy().into(),
                                    span: arg.as_span(),
                                };

                                if arg.as_rule() == Rule::vararg {
                                    vararg = true;
                                } else {
                                    let TypedName{typed, name, tags} = parse_named_type((file_str, n), arg);

                                    args.push(NamedArg{
                                        name,
                                        typed,
                                        tags,
                                        loc: argloc,
                                    });
                                }
                            }
                        },
                        Rule::block => {
                            body = Some(parse_block((file_str, n), part));
                        },
                        e => panic!("unexpected rule {:?} in function", e),
                    }
                }

                module.locals.push(Local{
                    name,
                    export_as,
                    vis,
                    loc,
                    def:Def::Function{
                        ret,
                        args,
                        body: body.unwrap(),
                        vararg,
                    }
                });
            },
            Rule::EOI => {},
            Rule::struct_d => {
                let decl = decl.into_inner();

                let mut vis    = Visibility::Object;
                let mut name   = None;
                let mut export_as = None;
                let mut fields = Vec::new();
                let mut loc    = None;
                let mut packed = false;

                for part in PP::new(n, decl) {
                    match part.as_rule() {
                        Rule::key_packed => {
                            packed = true;
                        }
                        Rule::key_shared => {
                            vis = Visibility::Shared;
                        }
                        Rule::exported => {
                            vis = Visibility::Export;
                            for part in part.into_inner() {
                                match part.as_rule() {
                                    Rule::ident => {
                                        export_as = Some(part.as_str().to_string());
                                    },
                                    e => panic!("unexpected rule {:?} in export", e),
                                }
                            }
                        }
                        Rule::ident => {
                            loc  = Some(Location{
                                file: n.to_string_lossy().into(),
                                span: part.as_span(),
                            });
                            name= Some(part.as_str().into());
                        }
                        Rule::struct_f => {

                            let loc  = Location{
                                file: n.to_string_lossy().into(),
                                span: part.as_span(),
                            };


                            let mut part = part.into_inner();

                            let TypedName{typed, name, tags} = parse_named_type((file_str, n), part.next().unwrap());

                            let array = match part.next() {
                                None => None,
                                Some(array) => {
                                    let expr = array.into_inner().next().unwrap();
                                    Some(parse_expr((file_str, n), expr))
                                }
                            };


                            fields.push(Field{
                                typed,
                                array,
                                tags,
                                name,
                                loc,
                            });
                        }
                        e => panic!("unexpected rule {:?} in struct ", e),
                    }
                };



                module.locals.push(Local{
                    name: name.unwrap(),
                    export_as,
                    vis,
                    loc: loc.unwrap(),
                    def: Def::Struct {
                        fields,
                        packed,
                    }
                });
            }
            Rule::import => {
                let loc  = Location{
                    file: n.to_string_lossy().into(),
                    span: decl.as_span(),
                };
                let mut vis = Visibility::Object;
                let mut importname = None;
                let mut alias      = None;
                for part in decl.into_inner() {
                    match part.as_rule() {
                        Rule::importname => {
                            importname = Some(parse_importname(part));
                        },
                        Rule::exported => {
                            vis = Visibility::Export;
                        }
                        Rule::importalias => {
                            alias = Some(part.into_inner().next().unwrap().as_str().to_string());
                        }
                        e => panic!("unexpected rule {:?} in import ", e),
                    }
                };

                let (name, local) = importname.unwrap();
                module.imports.push(Import{
                    name,
                    alias,
                    local,
                    vis,
                    loc
                });


            },
            Rule::comment => {},
            Rule::istatic | Rule::constant => {
                let rule = decl.as_rule();
                let loc  = Location{
                    file: n.to_string_lossy().into(),
                    span: decl.as_span(),
                };
                let mut storage = Storage::Static;
                let mut vis     = Visibility::Object;
                let mut typed   = None;
                let mut expr    = None;

                for part in decl.into_inner() {
                    match part.as_rule() {
                        Rule::key_thread_local => {
                            storage = Storage::ThreadLocal;
                        }
                        Rule::key_static => {
                            storage = Storage::Static;
                        }
                        Rule::key_atomic => {
                            storage = Storage::Atomic;
                        }
                        Rule::key_shared =>  {
                            if let Rule::istatic = rule {
                                let e = pest::error::Error::<Rule>::new_from_span(pest::error::ErrorVariant::CustomError {
                                    message: format!("cannot change visibility of static variable"),
                                }, part.as_span());
                                error!("{} : {}", n.to_string_lossy(), e);
                                std::process::exit(9);
                            } else {
                                vis = Visibility::Shared;
                            }
                        }
                        Rule::exported => {
                            if let Rule::istatic = rule {
                                let e = pest::error::Error::<Rule>::new_from_span(pest::error::ErrorVariant::CustomError {
                                    message: format!("cannot change visibility of static variable"),
                                }, part.as_span());
                                error!("{} : {}", n.to_string_lossy(), e);
                                std::process::exit(9);
                            } else {
                                vis = Visibility::Export;
                            }
                        },
                        Rule::named_type => {
                            typed = Some(parse_named_type((file_str, n), part));
                        },
                        Rule::expr if expr.is_none() => {
                            expr = Some(parse_expr((file_str, n), part));
                        }
                        e => panic!("unexpected rule {:?} in static", e),
                    }
                }

                let TypedName{typed, name, tags} = typed.unwrap();
                match rule {

                    Rule::constant => {
                        for (_,tag) in tags.0 {
                            error!("syntax error\n{}",
                                   make_error(&tag.iter().next().unwrap().1, "anonymous type cannot have storage tags (yet)"),
                                   );
                            std::process::exit(9);
                        }

                        module.locals.push(Local{
                            export_as: None,
                            name: name,
                            loc,
                            vis,
                            def: Def::Const {
                                typed,
                                expr: expr.unwrap(),
                            }
                        });
                    },
                    Rule::istatic => {
                        module.locals.push(Local{
                            export_as: None,
                            name: name,
                            loc,
                            vis: Visibility::Object,
                            def: Def::Static {
                                tags,
                                storage,
                                typed,
                                expr: expr.unwrap(),
                            }
                        });
                    },
                    _ => unreachable!(),
                }
            },
            e => panic!("unexpected rule {:?} in file", e),

        }

    }

    Ok(module)
}

pub(crate) fn parse_expr(n: (&'static str, &Path), decl: pest::iterators::Pair<'static, Rule>) -> Expression {
    match decl.as_rule() {
        Rule::lhs   => { }
        Rule::expr  => { }
        Rule::termish => { }
        _ => { panic!("parse_expr call called with {:?}", decl); }
    };

    let mut s_op = Some((String::new(), Location{
        file: n.1.to_string_lossy().into(),
        span: decl.as_span(),
    }));
    let mut s_r  = Vec::new();

    let decl = decl.into_inner();
    for expr in decl {
        let loc = Location{
            file: n.1.to_string_lossy().into(),
            span: expr.as_span(),
        };
        match expr.as_rule() {
            Rule::infix => {
                s_op = Some((expr.as_str().to_string(), loc));
            },
            Rule::unarypre => {
                let mut expr = expr.into_inner();
                let part    = expr.next().unwrap();
                let op      = part.as_str().to_string();
                let part   = expr.next().unwrap();
                let iexpr   = match part.as_rule() {
                    Rule::type_name => {
                        let loc = Location{
                            file: n.1.to_string_lossy().into(),
                            span: part.as_span(),
                        };
                        let name = Name::from(part.as_str());
                        Expression::Name(Typed{
                            ptr: Vec::new(),
                            name,
                            loc,
                        })
                    },
                    Rule::termish => {
                        parse_expr(n, part)
                    }
                    e => panic!("unexpected rule {:?} in unary pre lhs", e),
                };


                s_r.push((s_op.take().unwrap(), Box::new(Expression::UnaryPre{
                    expr: Box::new(iexpr),
                    op,
                    loc,
                })));
            },
            Rule::unarypost => {
                let mut expr = expr.into_inner();
                let part   = expr.next().unwrap();
                let iexpr   = match part.as_rule() {
                    Rule::type_name => {
                        let loc = Location{
                            file: n.1.to_string_lossy().into(),
                            span: part.as_span(),
                        };
                        let name = Name::from(part.as_str());
                        Expression::Name(Typed{
                            ptr: Vec::new(),
                            name,
                            loc,
                        })
                    },
                    Rule::expr => {
                        parse_expr(n, part)
                    }
                    e => panic!("unexpected rule {:?} in unary post lhs", e),
                };

                let part    = expr.next().unwrap();
                let op      = part.as_str().to_string();

                s_r.push((s_op.take().unwrap(), Box::new(Expression::UnaryPost{
                    expr: Box::new(iexpr),
                    op,
                    loc,
                })));
            },
            Rule::cast => {
                let exprloc = Location{
                    file: n.1.to_string_lossy().into(),
                    span: expr.as_span(),
                };
                let mut expr = expr.into_inner();
                let part  = expr.next().unwrap();
                let into = parse_anon_type(n, part);
                let part  = expr.next().unwrap();
                let expr = parse_expr(n, part);
                s_r.push((s_op.take().unwrap(), Box::new(Expression::Cast{
                    loc: exprloc,
                    into,
                    expr: Box::new(expr),
                })));
            },
            Rule::ptr_access | Rule::member_access | Rule::array_access => {
                let op = match expr.as_rule() {
                    Rule::ptr_access => "->",
                    Rule::member_access => ".",
                    Rule::array_access => "[",
                    _ => unreachable!(),
                }.to_string();
                let mut expr = expr.into_inner();

                let lhs;
                let e1  = expr.next().unwrap();
                match e1.as_rule() {
                    Rule::type_name => {
                        let loc = Location{
                            file: n.1.to_string_lossy().into(),
                            span: e1.as_span(),
                        };
                        let name = Name::from(e1.as_str());
                        lhs = Some(Expression::Name(Typed{
                            ptr: Vec::new(),
                            name,
                            loc,
                        }));
                    },
                    Rule::termish | Rule::expr  => {
                        lhs = Some(parse_expr(n, e1));
                    }
                    e => panic!("unexpected rule {:?} in access lhs", e),
                }

                if op == "[" {
                    let e2  = expr.next().unwrap();
                    match e2.as_rule() {
                        Rule::array => (),
                        _ => { panic!("unexpected rule {:?} in array expr", e2); }
                    };
                    let e2 = e2.into_inner().next().unwrap();
                    let rhs = parse_expr(n, e2);
                    s_r.push((s_op.take().unwrap(), Box::new(Expression::ArrayAccess{
                        lhs: Box::new(lhs.unwrap()),
                        rhs: Box::new(rhs),
                        loc,
                    })));
                } else {
                    let e2  = expr.next().unwrap();
                    let rhs = e2.as_str().to_string();
                    s_r.push((s_op.take().unwrap(), Box::new(Expression::MemberAccess{
                        lhs: Box::new(lhs.unwrap()),
                        rhs,
                        op,
                        loc,
                    })));
                }
            },
            Rule::type_name => {
                let name = Name::from(expr.as_str());
                s_r.push((s_op.take().unwrap(), Box::new(Expression::Name(Typed{
                    ptr: Vec::new(),
                    name,
                    loc,
                }))));
            },
            Rule::number_literal | Rule::string_literal | Rule::char_literal => {
                s_r.push((s_op.take().unwrap(), Box::new(Expression::Literal {
                    v: expr.as_str().to_string(),
                    loc,
                })));
            },
            Rule::expr => {
                s_r.push((s_op.take().unwrap(), Box::new(parse_expr(n, expr))));
            },
            Rule::deref | Rule::takeref => {
                let op = match expr.as_rule() {
                    Rule::deref   => "*",
                    Rule::takeref => "&",
                    _ => unreachable!(),
                }.to_string();

                let part = expr.into_inner().next().unwrap();
                let expr = match part.as_rule() {
                    Rule::type_name => {
                        let loc = Location{
                            file: n.1.to_string_lossy().into(),
                            span: part.as_span(),
                        };
                        let name = Name::from(part.as_str());
                        Expression::Name(Typed{
                            ptr: Vec::new(),
                            name,
                            loc,
                        })
                    },
                    Rule::termish => {
                        parse_expr(n, part)
                    }
                    e => panic!("unexpected rule {:?} in deref lhs", e),
                };
                s_r.push((s_op.take().unwrap(), Box::new(Expression::UnaryPre{
                    op,
                    loc,
                    expr: Box::new(expr),
                })));
            },
            Rule::call => {

                s_r.push((s_op.take().unwrap(), Box::new(parse_call(n, expr))));
            },
            Rule::array_init => {
                let mut fields = Vec::new();
                let expr = expr.into_inner();
                for part in expr {
                    match part.as_rule()  {
                        Rule::termish => {
                            let expr = parse_expr(n, part);
                            fields.push(Box::new(expr));
                        }
                        e => panic!("unexpected rule {:?} in struct init", e),
                    }

                }
                s_r.push((s_op.take().unwrap(), Box::new(Expression::ArrayInit{
                    loc,
                    fields,
                })));
            }
            Rule::struct_init => {
                let mut expr = expr.into_inner();
                let part  = expr.next().unwrap();
                let typloc = Location{
                    file: n.1.to_string_lossy().into(),
                    span: part.as_span(),
                };

                let typed = parse_anon_type(n, part);

                let mut fields = Vec::new();
                for part in expr {
                    match part.as_rule()  {
                        Rule::struct_init_field => {
                            let mut part = part.into_inner();
                            let name = part.next().unwrap().as_str().to_string();
                            let expr = parse_expr(n, part.next().unwrap());
                            fields.push((name, Box::new(expr)));
                        }
                        e => panic!("unexpected rule {:?} in struct init", e),
                    }

                }

                s_r.push((s_op.take().unwrap(), Box::new(Expression::StructInit{
                    loc,
                    typed,
                    fields,
                })));
            }
            e => panic!("unexpected rule {:?} in expr", e),
        }
    }



    let ((_,loc), lhs) = s_r.remove(0);
    if s_r.len() == 0 {
        return *lhs;
    }

    return Expression::InfixOperation {
        loc,
        lhs,
        rhs: s_r,
    }
}

pub(crate) fn parse_statement(n: (&'static str, &Path), stm: pest::iterators::Pair<'static, Rule>) -> Statement  {
    let loc = Location{
        file: n.1.to_string_lossy().into(),
        span: stm.as_span(),
    };
    match stm.as_rule() {
        Rule::mark_stm => {
            let mut stm = stm.into_inner();
            let part    = stm.next().unwrap();
            let lhs     = parse_expr(n, part);
            let part    = stm.next().unwrap();
            let tagloc = Location{
                file: n.1.to_string_lossy().into(),
                span: part.as_span(),
            };
            let mut part = part.into_inner();
            let key   = part.next().unwrap().as_str().into();
            let value = part.next().map(|s|s.as_str().into()).unwrap_or(String::new());

            Statement::Mark{
                loc,
                lhs,
                key,
                value,
            }
        },
        Rule::label => {
            let mut stm = stm.into_inner();
            let label   = stm.next().unwrap().as_str().to_string();
            Statement::Label{
                loc,
                label,
            }
        },
        Rule::continue_stm => {
            Statement::Continue{
                loc,
            }
        },
        Rule::break_stm => {
            Statement::Break{
                loc,
            }
        },
        Rule::goto_stm => {
            let mut stm = stm.into_inner();
            let label   = stm.next().unwrap().as_str().to_string();
            Statement::Goto{
                loc,
                label,
            }
        },
        Rule::block => {
            Statement::Block(Box::new(parse_block(n, stm)))
        },
        Rule::return_stm  => {
            let mut stm = stm.into_inner();
            let key  = stm.next().unwrap();
            match key.as_rule() {
                Rule::key_return => { }
                a => { panic!("expected key_return instead of {:?}", a );}
            };
            let expr = if let Some(expr) = stm.next() {
                Some(parse_expr(n, expr))
            } else {
                None
            };
            Statement::Return{
                expr,
                loc: loc.clone(),
            }
        },
        Rule::expr => {
            let expr = parse_expr(n, stm);
            Statement::Expr{
                expr,
                loc: loc.clone(),
            }
        }
        Rule::if_stm => {
            let mut stm = stm.into_inner();
            let part    = stm.next().unwrap();
            let expr    = parse_expr(n, part);
            let part    = stm.next().unwrap();
            let body    = parse_block(n, part);
            Statement::Cond{
                op: "if".to_string(),
                expr: Some(expr),
                body,
            }
        }
        Rule::elseif_stm => {
            let mut stm = stm.into_inner();
            let part    = stm.next().unwrap();
            let expr    = parse_expr(n, part);
            let part    = stm.next().unwrap();
            let body    = parse_block(n, part);
            Statement::Cond{
                op: "else if".to_string(),
                expr: Some(expr),
                body,
            }
        }
        Rule::else_stm => {
            let mut stm = stm.into_inner();
            let part    = stm.next().unwrap();
            let body    = parse_block(n, part);
            Statement::Cond{
                op: "else".to_string(),
                expr: None,
                body,
            }
        }
        Rule::for_stm => {



            let stm = stm.into_inner();

            let mut expr1 = None;
            let mut expr2 = None;
            let mut expr3 = None;
            let mut block = None;

            let mut cur = 1;

            for part in stm {
                match part.as_rule() {
                    Rule::semicolon => {
                        cur += 1;
                    },
                    Rule::block if cur == 3 && block.is_none() => {
                        block = Some(parse_block(n, part));
                    },
                    _ if cur == 1 => {
                        expr1 = Some(Box::new(parse_statement(n, part)));
                    },
                    _ if cur == 2 => {
                        expr2 = Some(Box::new(parse_statement(n, part)));
                    },
                    _ if cur == 3 => {
                        expr3 = Some(Box::new(parse_statement(n, part)));
                    },
                    e => panic!("unexpected rule {:?} in for ", e),
                }
            }

            Statement::For{
                e1:     expr1,
                e2:     expr2,
                e3:     expr3,
                body:   block.unwrap(),
            }
        }
        Rule::vardecl => {
            let stm = stm.into_inner();
            let mut typed   = None;
            let mut assign  = None;
            let mut array   = None;

            for part in stm {
                match part.as_rule() {
                    Rule::named_type => {
                        typed = Some(parse_named_type(n, part));
                    },
                    Rule::expr => {
                        assign = Some(parse_expr(n, part));
                    }
                    Rule::array => {
                        array = Some(parse_expr(n, part.into_inner().next().unwrap()));
                    }
                    e => panic!("unexpected rule {:?} in vardecl", e),
                }
            }

            let TypedName{typed, name, tags} = typed.unwrap();

            Statement::Var{
                loc: loc.clone(),
                typed,
                name,
                tags,
                array,
                assign,
            }
        }
        Rule::assign => {
            let stm = stm.into_inner();
            let mut lhs     = None;
            let mut rhs     = None;
            let mut op      = None;

            for part in stm {
                match part.as_rule() {
                    Rule::lhs if lhs.is_none() => {
                        lhs = Some(parse_expr(n, part));
                    }
                    Rule::assignop => {
                        op = Some(part.as_str().to_string());
                    }
                    Rule::expr if rhs.is_none() => {
                        rhs = Some(parse_expr(n, part));
                    }
                    e => panic!("unexpected rule {:?} in assign", e),
                }
            }

            Statement::Assign{
                loc:    loc.clone(),
                lhs:    lhs.unwrap(),
                rhs:    rhs.unwrap(),
                op:     op.unwrap(),
            }
        }
        e => panic!("unexpected rule {:?} in block", e),
    }
}

pub(crate) fn parse_block(n: (&'static str, &Path), decl: pest::iterators::Pair<'static, Rule>) -> Block {
    match decl.as_rule() {
        Rule::block => { }
        _ => { panic!("parse_block called with {:?}", decl); }
    };

    let end = Location{
        file: n.1.to_string_lossy().into(),
        span: pest::Span::new(n.0, decl.as_span().end(), decl.as_span().end()).unwrap(),
    };

    let mut statements = Vec::new();
    for stm in PP::new(n.1, decl.into_inner()) {
        statements.push(parse_statement(n, stm));
    }
    Block{
        statements,
        end,
    }
}


// typed is parsed left to right

#[derive(Debug)]
pub(crate) struct TypedName {
    name:   String,
    typed:  Typed,
    tags:   Tags,
}

pub(crate) fn parse_named_type(n: (&'static str, &Path), decl: pest::iterators::Pair<'static, Rule>) -> TypedName {
    match decl.as_rule() {
        Rule::named_type => { }
        _ => { panic!("parse_named_type called with {:?}", decl); }
    };

    let loc = Location{
        file: n.1.to_string_lossy().into(),
        span: decl.as_span(),
    };
    //the actual type name is always on the left hand side
    let mut decl = decl.into_inner();
    let typename = Name::from(decl.next().unwrap().as_str());

    // the local variable name is on the right;
    let mut decl : Vec<pest::iterators::Pair<'static, Rule>> = decl.collect();
    let name_part = decl.pop().unwrap();
    let name = match name_part.as_rule() {
        Rule::ident => {
            name_part.as_str().to_string()
        }
        _ => {
            let loc = Location{
                file: n.1.to_string_lossy().into(),
                span: name_part.as_span(),
            };
            error!("syntax error\n{}",
                   make_error(&loc, "expected a name"),
                   );
            std::process::exit(9);
        }
    };

    let mut tags = Tags::new();
    let mut ptr = Vec::new();

    for part in decl {
        let loc = Location{
            file: n.1.to_string_lossy().into(),
            span: part.as_span(),
        };
        match part.as_rule() {
            Rule::ptr => {
                ptr.push(Pointer{
                    tags: std::mem::replace(&mut tags, Tags::new()),
                    loc,
                });
            }
            Rule::key_mut  => {
                tags.insert(
                    "mutable".to_string(),
                    String::new(),
                    loc,
                );
            },
            Rule::tag_name => {
                let mut part = part.into_inner();
                let name  = part.next().unwrap().as_str().into();
                let value = part.next().as_ref().map(|s|s.as_str().to_string()).unwrap_or(String::new());
                tags.insert(name, value, loc);
            }
            e => panic!("unexpected rule {:?} in named_type ", e),
        }
    }

    TypedName {
        name,
        typed: Typed {
            name: typename,
            loc: loc.clone(),
            ptr,
        },
        tags,
    }
}

pub(crate) fn parse_anon_type(n: (&'static str, &Path), decl: pest::iterators::Pair<'static, Rule>) -> Typed {
    match decl.as_rule() {
        Rule::anon_type => { }
        _ => { panic!("parse_anon_type called with {:?}", decl); }
    };

    let loc = Location{
        file: n.1.to_string_lossy().into(),
        span: decl.as_span(),
    };
    //the actual type name is always on the left hand side
    let mut decl = decl.into_inner();
    let name = Name::from(decl.next().unwrap().as_str());

    let mut tags = Tags::new();
    let mut ptr = Vec::new();

    for part in decl {
        let loc = Location{
            file: n.1.to_string_lossy().into(),
            span: part.as_span(),
        };
        match part.as_rule() {
            Rule::ptr => {
                ptr.push(Pointer{
                    tags: std::mem::replace(&mut tags, Tags::new()),
                    loc,
                });
            }
            Rule::key_mut  => {
                tags.insert(
                    "mutable".to_string(),
                    String::new(),
                    loc,
                );
            },
            Rule::tag_name => {
                let mut part = part.into_inner();
                let name  = part.next().unwrap().as_str().into();
                let value = part.next().as_ref().map(|s|s.as_str().to_string()).unwrap_or(String::new());
                tags.insert(name, value, loc);
            }
            e => panic!("unexpected rule {:?} in anon_type", e),
        }
    }

    for (_,tag) in tags.0 {
        error!("syntax error\n{}",
               make_error(&tag.iter().next().unwrap().1, "anonymous type cannot have storage tags (yet)"),
               );
        std::process::exit(9);
    }

    Typed {
        name, loc, ptr,
    }
}


pub(crate) fn parse_importname(decl: pest::iterators::Pair<Rule>) -> (Name, Vec<(String, Option<String>)>) {
    let mut locals = Vec::new();
    let mut v = Vec::new();
    for part in decl.into_inner() {
        match part.as_rule() {
            Rule::cimport => {
                v = vec![String::new(), "ext".into(), part.as_str().into()];
            }
            Rule::ident => {
                v.push(part.as_str().into());
            }
            Rule::local => {
                for p2 in part.into_inner() {
                    match p2.as_rule() {
                        Rule::local_i => {
                            let mut p2      = p2.into_inner();
                            let name        = p2.next().unwrap();
                            let name = match name.as_rule() {
                                Rule::ident => {
                                    name.as_str().to_string()
                                }
                                Rule::qident => {
                                    name.into_inner().next().unwrap().as_str().to_string()
                                },
                                _ => unreachable!(),
                            };
                            let import_as   = if let Some(p3) = p2.next() {
                                Some(p3.as_str().to_string())
                            } else {
                                None
                            };
                            locals.push((name, import_as));
                        },
                        e => panic!("unexpected rule {:?} in local", e)
                    }
                }
            },
            Rule::type_name | Rule::importname => {
                let (name, locals2) = parse_importname(part);
                v.extend(name.0);
                locals.extend(locals2);
            }
            e => panic!("unexpected rule {:?} in import name ", e),
        }
    }
    (Name(v), locals)
}

fn parse_call(n: (&'static str, &Path), expr: pest::iterators::Pair<'static, Rule>) -> Expression {
    let loc = Location{
        file: n.1.to_string_lossy().into(),
        span: expr.as_span(),
    };
    let mut expr = expr.into_inner();
    let name = expr.next().unwrap();
    let nameloc = Location{
        file: n.1.to_string_lossy().into(),
        span: name.as_span(),
    };
    let name = Name::from(name.as_str());


    let mut args = Vec::new();


    for part in expr.into_iter() {
        match part.as_rule() {
            Rule::call_args => {
                args = part.into_inner().into_iter().map(|arg|{
                    Box::new(parse_expr(n, arg))
                }).collect();
            },
            e => panic!("unexpected rule {:?} in function call", e),
        }
    };

    Expression::Call{
        loc: loc,
        name: Typed{
            name,
            loc: nameloc,
            ptr: Vec::new(),
        },
        args,
    }
}


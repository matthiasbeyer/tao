use super::*;

pub use ast::Literal as Literal;

pub trait Meta {
    type Ty;
    type Data: Clone + fmt::Debug;
    type Class;
    type Global;
    type Effect;
}

impl Meta for InferMeta {
    type Ty = TyVar;
    type Data = SrcNode<DataId>;
    type Class = ClassVar;
    type Global = (DefId, Vec<Self>);
    type Effect = EffectVar;
}

impl Meta for TyMeta {
    type Ty = TyId;
    type Data = SrcNode<DataId>;
    type Class = Option<(ClassId, Vec<TyId>)>; // Required because we don't have proper error classes yet
    type Global = (DefId, Vec<Self>);
    type Effect = EffectId;
}

impl Meta for ConMeta {
    type Ty = ConTyId;
    type Data = ConDataId;
    type Class = !;
    type Global = ConProcId;
    type Effect = ConEffectId;
}

#[derive(Clone, Debug)]
pub enum Pat<M: Meta> {
    Error,
    Wildcard,
    Literal(Literal),
    Single(Node<Binding<M>, M>),
    Add(Node<Binding<M>, M>, SrcNode<u64>),
    Tuple(Vec<Node<Binding<M>, M>>),
    Record(BTreeMap<Ident, Node<Binding<M>, M>>),
    ListExact(Vec<Node<Binding<M>, M>>),
    ListFront(Vec<Node<Binding<M>, M>>, Option<Node<Binding<M>, M>>),
    Decons(M::Data, Ident, Node<Binding<M>, M>),
}

#[derive(Clone, Debug)]
pub struct Binding<M: Meta> {
    pub pat: SrcNode<Pat<M>>,
    pub name: Option<SrcNode<Ident>>,
}

pub type InferBinding = InferNode<Binding<InferMeta>>;
pub type TyBinding = TyNode<Binding<TyMeta>>;
pub type ConBinding = ConNode<Binding<ConMeta>>;

impl<M: Meta> Binding<M> {
    pub fn from_pat(pat: SrcNode<Pat<M>>) -> Self {
        Self { pat, name: None }
    }

    pub fn wildcard(name: SrcNode<Ident>) -> Self {
        Self { pat: SrcNode::new(hir::Pat::Wildcard, name.span()), name: Some(name) }
    }

    pub fn unit(span: Span) -> Self {
        Self { pat: SrcNode::new(hir::Pat::Tuple(Vec::new()), span), name: None }
    }
}

impl Binding<InferMeta> {
    pub fn get_binding_tys(self: &InferNode<Self>) -> Vec<(SrcNode<Ident>, TyVar)> {
        let mut bindings = Vec::new();
        self.visit_bindings_inner(&mut |name, (_, ty)| bindings.push((name.clone(), *ty)));
        bindings
    }
}

impl<M: Meta> Binding<M> {
    fn visit_bindings_inner(self: &Node<Self, M>, visit: &mut impl FnMut(&SrcNode<Ident>, &M)) {
        // TODO: Check for duplicates!
        if let Some(name) = &self.name { visit(name, self.meta()); };
        match &*self.pat {
            Pat::Error => {},
            Pat::Wildcard => {},
            Pat::Literal(_) => {},
            Pat::Single(inner) => inner.visit_bindings_inner(visit),
            Pat::Add(lhs, _) => lhs.visit_bindings_inner(visit),
            Pat::Tuple(items) => items
                .iter()
                .for_each(|item| item.visit_bindings_inner(visit)),
            Pat::Record(fields) => fields
                .values()
                .for_each(|field| field.visit_bindings_inner(visit)),
            Pat::ListExact(items) => items
                .iter()
                .for_each(|item| item.visit_bindings_inner(visit)),
            Pat::ListFront(items, tail) => {
                items
                    .iter()
                    .for_each(|item| item.visit_bindings_inner(visit));
                if let Some(tail) = tail { tail.visit_bindings_inner(visit); }
            },
            Pat::Decons(_, _, inner) => inner.visit_bindings_inner(visit),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Intrinsic {
    TypeName,
    NegNat,
    NegInt,
    NegReal,
    EqChar,
    EqNat,
    LessNat,
    AddNat,
    MulNat,
    Go,
    Print,
    Input,
    LenList,
    SkipList,
    TrimList,
    JoinList,
    Propagate,
}

#[derive(Debug)]
pub enum Expr<M: Meta> {
    Error,
    Literal(Literal),
    // TODO: replace with `Item` when scoping is added
    Local(Ident),
    Global(M::Global),
    Tuple(Vec<Node<Self, M>>),
    List(Vec<Node<Self, M>>, Vec<Node<Self, M>>),
    Record(Vec<(SrcNode<Ident>, Node<Self, M>)>),
    Access(Node<Self, M>, SrcNode<Ident>),
    // hidden_outer
    Match(bool, Node<Self, M>, Vec<(Node<Binding<M>, M>, Node<Self, M>)>),
    Func(Node<Ident, M>, Node<Self, M>),
    Apply(Node<Self, M>, Node<Self, M>),
    Cons(M::Data, Ident, Node<Self, M>),

    // member, class, field
    ClassAccess(M, M::Class, SrcNode<Ident>),

    Intrinsic(SrcNode<Intrinsic>, Vec<Node<Self, M>>),
    Update(Node<Self, M>, Vec<(SrcNode<Ident>, Node<Self, M>)>),

    // Blocks propagation of effects, collecting them
    // i.e: `@{ foo?; bar?; x }` gets type `foo + bar ~ X`
    Basin(M::Effect, Node<Self, M>),
    Suspend(M::Effect, Node<Self, M>),
    Handle {
        expr: Node<Self, M>,
        eff: M::Effect,
        send: Node<Ident, M>,
        recv: Node<Self, M>,
    },
}

pub type InferExpr = InferNode<Expr<InferMeta>>;
pub type TyExpr = TyNode<Expr<TyMeta>>;
pub type ConExpr = ConNode<Expr<ConMeta>>;

impl Expr<ConMeta> {
    fn required_locals_inner(&self, stack: &mut Vec<Ident>, required: &mut Vec<Ident>) {
        match self {
            Expr::Literal(_) | Expr::Error => {},
            Expr::Local(local) => {
                if !stack.contains(local) {
                    required.push(*local);
                }
            },
            Expr::Global(_) => {},
            Expr::Intrinsic(_, args) => args
                .iter()
                .for_each(|arg| arg.required_locals_inner(stack, required)),
            Expr::Match(_, pred, arms) => {
                pred.required_locals_inner(stack, required);
                for (arm, body) in arms {
                    let old_stack = stack.len();
                    arm.visit_bindings_inner(&mut |name, _| stack.push(**name));
                    body.required_locals_inner(stack, required);
                    stack.truncate(old_stack);
                }
            },
            Expr::Func(arg, body) => {
                stack.push(**arg);
                body.required_locals_inner(stack, required);
                stack.pop();
            },
            Expr::Apply(f, arg) => {
                f.required_locals_inner(stack, required);
                arg.required_locals_inner(stack, required);
            },
            Expr::Tuple(fields) => fields
                .iter()
                .for_each(|field| field.required_locals_inner(stack, required)),
            Expr::Record(fields) => fields
                .iter()
                .for_each(|(_, field)| field.required_locals_inner(stack, required)),
            Expr::List(items, tails) => {
                items
                    .iter()
                    .for_each(|item| item.required_locals_inner(stack, required));
                tails
                    .iter()
                    .for_each(|tail| tail.required_locals_inner(stack, required));
            },
            Expr::Access(tuple, _) => tuple.required_locals_inner(stack, required),
            Expr::Cons(_, _, inner) => inner.required_locals_inner(stack, required),
            Expr::Access(inner, _) => inner.required_locals_inner(stack, required),
            Expr::ClassAccess(_, _, _) => {},
            Expr::Update(record, fields) => {
                record.required_locals_inner(stack, required);
                fields
                    .iter()
                    .for_each(|(_, field)| field.required_locals_inner(stack, required));
            },
            Expr::Basin(_, inner) => inner.required_locals_inner(stack, required),
            Expr::Suspend(_, inner) => inner.required_locals_inner(stack, required),
            Expr::Handle { expr, send, recv, .. } => {
                expr.required_locals_inner(stack, required);

                stack.push(**send);
                recv.required_locals_inner(stack, required);
                stack.pop();
            },
        }
    }

    pub fn required_locals(&self, already_has: impl IntoIterator<Item = Ident>) -> Vec<Ident> {
        let mut required = Vec::new();
        self.required_locals_inner(&mut already_has.into_iter().collect(), &mut required);
        required
    }
}

use super::*;

pub struct Data {
    pub name: SrcNode<Ident>,
    pub attr: Vec<SrcNode<ast::Attr>>,
    pub gen_scope: GenScopeId,
    pub cons: Vec<(SrcNode<Ident>, TyId)>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DataId(usize);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AliasId(usize);

pub struct Alias {
    pub name: Ident,
    pub attr: Vec<SrcNode<ast::Attr>>,
    pub gen_scope: GenScopeId,
    pub ty: TyId,
}

#[derive(Default)]
pub struct Lang {
    pub go: Option<DataId>,
}

#[derive(Default)]
pub struct Datas {
    // TODO: Don't use `Result`
    name_lut: HashMap<Ident, (Span, Result<DataId, AliasId>, GenScopeId)>,
    cons_lut: HashMap<Ident, (Span, DataId)>,
    alias_lut: HashMap<Ident, Alias>,
    datas: Vec<(Span, Option<Data>)>,
    aliases: Vec<(Span, Option<Alias>)>,
    pub lang: Lang,
}

impl Datas {
    pub fn name_gen_scope(&self, name: Ident) -> GenScopeId {
        self.name_lut[&name].2
    }

    pub fn lookup_data(&self, name: Ident) -> Option<DataId> {
        self.name_lut
            .get(&name)
            .and_then(|data| data.1.as_ref().ok())
            .copied()
    }

    pub fn lookup_alias(&self, name: Ident) -> Option<AliasId> {
        self.name_lut
            .get(&name)
            .and_then(|data| data.1.as_ref().err())
            .copied()
    }

    pub fn lookup_cons(&self, name: Ident) -> Option<DataId> {
        self.cons_lut.get(&name).map(|(_, id)| *id)
    }

    pub fn get_data(&self, data: DataId) -> &Data {
        self.datas[data.0]
            .1
            .as_ref()
            .expect("Declared data accessed before being defined")
    }

    pub fn get_data_span(&self, data: DataId) -> Span {
        self.datas[data.0].0
    }

    pub fn get_alias(&self, alias: AliasId) -> Option<&Alias> {
        self.aliases[alias.0]
            .1
            .as_ref()
    }

    pub fn get_alias_span(&self, alias: AliasId) -> Span {
        self.aliases[alias.0].0
    }

    pub fn declare_data(&mut self, name: SrcNode<Ident>, gen_scope: GenScopeId, attr: &[SrcNode<ast::Attr>]) -> Result<DataId, Error> {
        let id = DataId(self.datas.len());
        if let Err(old) = self.name_lut.try_insert(*name, (name.span(), Ok(id), gen_scope)) {
            Err(Error::DuplicateTypeName(*name, old.entry.get().0, name.span()))
        } else {
            if let Some(lang) = attr
                .iter()
                .find(|a| &**a.name == "lang")
                .and_then(|a| a.args.as_ref())
            {
                if lang.iter().find(|a| &**a.name == "go").is_some() {
                    self.lang.go = Some(id);
                }
            }

            self.datas.push((name.span(), None));
            Ok(id)
        }
    }

    pub fn check_lang_items(&self) -> Vec<Error> {
        let mut errors = Vec::new();

        if self.lang.go.is_none() { errors.push(Error::MissingLangItem("go")); }

        errors
    }

    pub fn declare_alias(&mut self, name: Ident, span: Span, gen_scope: GenScopeId) -> Result<AliasId, Error> {
        let id = AliasId(self.aliases.len());
        if let Err(old) = self.name_lut.try_insert(name, (span, Err(id), gen_scope)) {
            Err(Error::DuplicateTypeName(name, old.entry.get().0, span))
        } else {
            self.aliases.push((span, None));
            Ok(id)
        }
    }

    pub fn define_data(&mut self, id: DataId, span: Span, data: Data) -> Result<(), Vec<Error>> {
        let mut errors = Vec::new();
        for (cons, _) in &data.cons {
            if let Err(old) = self.cons_lut.try_insert(**cons, (cons.span(), id)) {
                errors.push(Error::DuplicateConsName(**cons, old.entry.get().0, cons.span()));
            }
        }
        self.datas[id.0].1 = Some(data);
        if errors.len() == 0 {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn define_alias(&mut self, id: AliasId, alias: Alias) {
        self.aliases[id.0].1 = Some(alias);
    }
}

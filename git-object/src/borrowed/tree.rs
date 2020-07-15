use crate::{borrowed::parse::SPACE, borrowed::Error};
use crate::{BStr, ByteSlice};
use nom::{
    bytes::complete::{tag, take, take_while1, take_while_m_n},
    character::is_digit,
    combinator::all_consuming,
    multi::many1,
    sequence::terminated,
    IResult,
};
use std::convert::TryFrom;

#[derive(PartialEq, Eq, Debug, Hash, Ord, PartialOrd, Clone)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct Tree<'a> {
    #[cfg_attr(feature = "serde1", serde(borrow))]
    pub entries: Vec<Entry<'a>>,
}

#[derive(PartialEq, Eq, Debug, Hash, Ord, PartialOrd, Clone)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct Entry<'a> {
    pub mode: Mode,
    pub filename: &'a BStr,
    /// a 20 bytes SHA1
    pub oid: &'a [u8],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Ord, PartialOrd, Hash)]
#[repr(u16)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub enum Mode {
    Tree = 0o040000u16,
    Blob = 0o100644,
    BlobExecutable = 0o100755,
    Link = 0o120000,
    Commit = 0o160000,
}

impl TryFrom<&[u8]> for Mode {
    type Error = Error;

    fn try_from(mode: &[u8]) -> Result<Self, Self::Error> {
        Ok(match mode {
            b"40000" => Mode::Tree,
            b"100644" => Mode::Blob,
            b"100755" => Mode::BlobExecutable,
            b"120000" => Mode::Link,
            b"160000" => Mode::Commit,
            _ => return Err(Error::NomDetail(mode.into(), "unknown tree mode")),
        })
    }
}

const NULL: &[u8] = b"\0";
fn parse_entry(i: &[u8]) -> IResult<&[u8], Entry, Error> {
    let (i, mode) = terminated(take_while_m_n(5, 6, is_digit), tag(SPACE))(i)?;
    let mode = Mode::try_from(mode).map_err(nom::Err::Error)?;
    let (i, filename) = terminated(take_while1(|b| b != NULL[0]), tag(NULL))(i)?;
    let (i, oid) = take(20u8)(i)?;

    Ok((
        i,
        Entry {
            mode,
            filename: filename.as_bstr(),
            oid,
        },
    ))
}

fn parse(i: &[u8]) -> IResult<&[u8], Tree, Error> {
    let (i, mut entries) = all_consuming(many1(parse_entry))(i)?;
    entries.shrink_to_fit();
    Ok((i, Tree { entries }))
}

impl<'a> Tree<'a> {
    pub fn from_bytes(d: &'a [u8]) -> Result<Tree<'a>, Error> {
        parse(d).map(|(_, t)| t).map_err(Error::from)
    }
}

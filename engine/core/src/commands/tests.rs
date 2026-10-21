// SPDX-License-Identifier: GPL-3.0-or-later
//! Conformance tests of command-system.md §9 (numbers in the test names).

use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::variant::{Elem, Variant};

/// A test converter, exact for the values these tests use (strings, integers, Empty); the
/// full one is `fc4_formats::command_values`.
struct TestConv;
impl ValueConvert for TestConv {
    fn get_val_string(&self, v: &Variant, out: &mut Vec<u8>) -> bool {
        match v {
            Variant::Array { elem: 0x0E, .. } => {
                *out = v.bytes().unwrap();
                true
            }
            Variant::Scalar(Elem::I32(n)) => {
                *out = n.to_string().into_bytes();
                false
            }
            Variant::Scalar(Elem::U32(n)) => {
                *out = n.to_string().into_bytes();
                false
            }
            Variant::Empty => {
                out.clear();
                false
            }
            _ => false,
        }
    }
    fn get_val_sint32(&self, v: &Variant) -> Option<i32> {
        match v {
            Variant::Scalar(Elem::I32(n)) => Some(*n),
            Variant::Array { elem: 0x0E, .. } => {
                Some(std::str::from_utf8(&v.bytes().unwrap()).ok()?.parse().unwrap_or(0))
            }
            _ => Some(0),
        }
    }
}

fn s(t: &str) -> Variant {
    Variant::string(t.as_bytes())
}
fn i32v(n: i32) -> Variant {
    Variant::Scalar(Elem::I32(n))
}
fn conv() -> Rc<dyn ValueConvert> {
    Rc::new(TestConv)
}

#[test]
fn t01_empty_set() {
    let p = CommandParameterSet::new();
    assert_eq!(p.get_parameter_count(), 0);
    assert_eq!(p.get_first_parameter_id(), NO_PARAMETER_ID);
    assert!(p.get_parameter(0).is_none());
    assert_eq!(*p.get_or_fallback(0), Variant::Empty);
    assert_eq!(p.get_parameter_count(), 0);
}

#[test]
fn t02_append_numbering() {
    let mut p = CommandParameterSet::new();
    for _ in 0..3 {
        p.append_parameter(&s("a"));
    }
    assert_eq!(p.ids(), vec![0, 1, 2]);
    p.set_parameter(10, &Variant::Scalar(Elem::U32(7)));
    p.append_parameter(&s("a"));
    assert!(p.get_parameter(11).is_some());
    p.set_status_parameter_value(5);
    assert_eq!(p.get_parameter(STATUS_PARAMETER_ID), Some(&i32v(5)));
    p.append_parameter(&s("a"));
    assert!(p.get_parameter(0x2B99_9D2C).is_some());
    assert_eq!(p.get_parameter_count(), 7);
    // Wrap: after 0xFFFFFFFF the append overwrites 0 and the count does not grow.
    let mut w = CommandParameterSet::new();
    w.set_parameter(0, &s("x"));
    w.set_parameter(u32::MAX, &s("y"));
    w.append_parameter(&s("z"));
    assert_eq!(w.get_parameter_count(), 2);
    assert_eq!(w.get_parameter(0), Some(&s("z")));
    w.set_parameter_count(5); // [choice]: terminates
    assert_eq!(w.get_parameter_count(), 2);
}

#[test]
fn t03_count() {
    let mut p = CommandParameterSet::new();
    for id in [0, 1, 2, 10] {
        p.set_parameter(id, &s("v"));
    }
    p.set_parameter_count(2);
    assert_eq!(p.ids(), vec![0, 1]);
    let mut q = CommandParameterSet::new();
    q.set_parameter(0, &s("v"));
    q.set_parameter(5, &s("v"));
    q.set_parameter_count(4);
    assert_eq!(q.ids(), vec![0, 5, 6, 7]);
    assert_eq!(q.get_parameter(6), Some(&Variant::Empty));
    let mut r = CommandParameterSet::new();
    r.set_parameter(0, &s("v"));
    r.set_status_parameter_value(0);
    r.set_parameter_count(1);
    assert_eq!(r.ids(), vec![0]);
}

#[test]
fn t04_iteration() {
    let mut p = CommandParameterSet::new();
    p.set_parameter(0, &s("v"));
    p.set_parameter(5, &s("v"));
    assert_eq!(p.get_first_parameter_id(), 0);
    assert_eq!(p.get_next_parameter_id(0), 5);
    assert_eq!(p.get_next_parameter_id(5), NO_PARAMETER_ID);
    assert_eq!(p.get_next_parameter_id(3), NO_PARAMETER_ID);
}

#[test]
fn t05_fallback() {
    let mut p = CommandParameterSet::new();
    *p.get_or_fallback_mut(99) = i32v(4);
    assert_eq!(*p.get_or_fallback(98), i32v(4));
    p.set_parameter_count(0);
    assert_eq!(*p.get_or_fallback(98), i32v(4));
    assert!(p.get_parameter(98).is_none());
}

#[test]
fn t06_remove_and_copy() {
    let mut p = CommandParameterSet::new();
    p.set_parameter(0, &s("a"));
    p.remove_parameter(42);
    assert_eq!(p.ids(), vec![0]);
    let mut v = s("xy");
    p.set_parameter(1, &v);
    v = s("changed");
    assert_ne!(v, s("xy"));
    assert_eq!(p.get_parameter(1), Some(&s("xy")));
}

#[test]
fn t07_compare() {
    let c = TestConv;
    let mut p = CommandParameterSet::new();
    p.set_parameter(0, &s("ABC"));
    p.set_parameter(1, &i32v(5));
    p.set_parameter(2, &s(""));
    assert!(p.compare_parameter_string(0, b"abc", false, &c));
    assert!(!p.compare_parameter_string(0, b"abc", true, &c));
    assert!(!p.compare_parameter_string(0, b"AB", false, &c));
    assert!(!p.compare_parameter_string(1, b"5", false, &c));
    assert!(p.compare_parameter_string(2, b"", true, &c));
    assert!(!p.compare_parameter_string(7, b"x", false, &c));
    assert!(p.compare_parameter_string(ANY_PARAMETER_ID, b"abc", false, &c));
    assert!(!p.compare_parameter_string(ANY_PARAMETER_ID, b"zzz", false, &c));
}

#[test]
fn t08_validate() {
    let mut p = CommandParameterSet::new();
    p.set_parameter(0, &Variant::Empty);
    assert!(p.validate_parameter(0, 0xFF));
    assert!(!p.validate_parameter(0, 7));
    assert!(!p.validate_parameter(1, 0xFF));
}

fn split(t: &str) -> Vec<String> {
    let (ok, w) = parse_delimited_text(t.as_bytes(), b' ', 64);
    assert!(ok);
    w.into_iter().map(|x| String::from_utf8(x).unwrap()).collect()
}

#[test]
fn t09_split() {
    assert!(split("").is_empty());
    assert!(split(" \t ").is_empty());
    assert_eq!(split("  SetDebugLevel   5  "), ["SetDebugLevel", "5"]);
    assert_eq!(split("a\tb"), ["a", "b"]);
    assert_eq!(split("say \"hello world\""), ["say", "hello world"]);
    assert_eq!(split("\"List Commands\" x"), ["List Commands", "x"]);
    assert_eq!(split("x \"\" y"), ["x", "", "y"]);
    assert_eq!(split("a \"b\tc\" d"), ["a", "b\tc", "d"]);
    assert_eq!(split("x \"a b\" \"c\"d"), ["x", "a b", "\"c\"d"]);
    assert_eq!(split("\"a\"b\""), ["a\"b"]);
    assert_eq!(split("a \"b"), ["a", "\"b"]);
    assert_eq!(split("\"abc def"), ["\"abc def"]);
    assert_eq!(split("a\"b c\"d"), ["a\"b c\"d"]);
    assert_eq!(split("\""), [""]);
    assert_eq!(split("a \"b  "), ["a", "\"b  "]);
    assert_eq!(split("a\nb\rc"), ["a\nb\rc"]);
    let (ok, w) = parse_delimited_text(b"a b", b' ', 1);
    assert!(!ok);
    assert_eq!(w, vec![b"a".to_vec()]);
    let (ok, w) = parse_delimited_text(b"a  ", b' ', 1);
    assert!(ok);
    assert_eq!(w, vec![b"a".to_vec()]);
    let (ok, w) = parse_delimited_text(b"a, b ,,c  ", b',', 8);
    assert!(ok);
    assert_eq!(w, vec![b"a".to_vec(), b"b".to_vec(), Vec::new(), b"c  ".to_vec()]);
    let (_, w) = parse_delimited_text(b" x , \"y\" ", b',', 8);
    assert_eq!(w, vec![b"x".to_vec(), b"\"y\" ".to_vec()]);
    assert_eq!(line_of(b"ab\0cd", u32::MAX), b"ab");
    assert_eq!(line_of(b"ab\0cd", 4), b"ab\0c");
}

/// A scripted command server: a name table and a dispatcher.
struct Server {
    names: Vec<(u32, &'static str)>,
    dispatcher: RefCell<Option<Message2CommandDispatcher>>,
}
impl CommandServer for Server {
    fn get_command_id(&self, name: &[u8]) -> u32 {
        let mut v = self.names.clone();
        v.sort();
        v.iter()
            .find(|(_, n)| make_lower(n.as_bytes()) == make_lower(name))
            .map_or(0, |e| e.0)
    }
    fn get_command_name(&self, id: u32) -> Option<Vec<u8>> {
        self.names.iter().find(|e| e.0 == id).map(|e| e.1.as_bytes().to_vec())
    }
    fn execute_command_id(&self, id: u32, i: Option<&SetRef>, o: Option<&SetRef>) -> i32 {
        match self.dispatcher.borrow_mut().as_mut() {
            Some(d) => d.execute_command(id, i, o),
            None => 2,
        }
    }
}

fn server() -> Rc<Server> {
    Rc::new(Server {
        names: vec![
            (0x2BB4_0833, "SetDebugLevel"),
            (0x20, "DUP"),
            (0x10, "dup"),
            (0x30, "My Cmd"),
        ],
        dispatcher: RefCell::new(None),
    })
}

fn generator(srv: &Rc<Server>) -> StringCommandGenerator {
    let mut g = StringCommandGenerator::new(conv());
    g.init(Some(srv.clone() as Rc<dyn CommandServer>));
    g
}

#[test]
fn t10_text_to_command() {
    let srv = server();
    let mut g = generator(&srv);
    let (mut id, mut p) = (7u32, None);
    assert!(g.convert_string_to_command(b"  setdebuglevel   5  ", u32::MAX, &mut id, &mut p));
    assert_eq!(id, 0x2BB4_0833);
    let set = p.take().unwrap();
    assert_eq!(set.borrow().ids(), vec![0]);
    assert_eq!(set.borrow().get_parameter(0), Some(&s("5")));
    assert_eq!(set.borrow().ref_count(), 1);
    assert!(g.convert_string_to_command(b"Dup", u32::MAX, &mut id, &mut p));
    assert_eq!(id, 0x10);
    assert_eq!(p.take().unwrap().borrow().get_parameter_count(), 0);
    assert!(g.convert_string_to_command(b"\"my cmd\" a \"b c\"", u32::MAX, &mut id, &mut p));
    assert_eq!(id, 0x30);
    let set = p.take().unwrap();
    assert_eq!(set.borrow().get_parameter(1), Some(&s("b c")));
    assert!(!g.convert_string_to_command(b"Nope 1", u32::MAX, &mut id, &mut p));
    assert_eq!(id, 0);
    assert!(p.is_none());
    id = 9;
    assert!(!g.convert_string_to_command(b"", u32::MAX, &mut id, &mut p));
    assert!(!g.convert_string_to_command(b"   ", u32::MAX, &mut id, &mut p));
    assert_eq!(id, 9);
    let mut line = String::from("SetDebugLevel");
    for k in 1..300 {
        line.push_str(&format!(" {k}"));
    }
    assert!(g.convert_string_to_command(line.as_bytes(), u32::MAX, &mut id, &mut p));
    let set = p.take().unwrap();
    assert_eq!(set.borrow().get_parameter_count(), 255);
    assert_eq!(set.borrow().get_parameter(254), Some(&s("255")));
    let mut none = StringCommandGenerator::new(conv());
    assert!(!none.convert_string_to_command(b"SetDebugLevel", u32::MAX, &mut id, &mut p));
    assert_eq!(none.execute_command(b"SetDebugLevel", u32::MAX, None), 2);
}

#[test]
fn t11_t12_to_text() {
    let srv = server();
    let g = generator(&srv);
    let p = CommandParameterSet::new_ref();
    p.borrow_mut().set_parameter(0, &s("a b"));
    p.borrow_mut().set_parameter(1, &i32v(7));
    p.borrow_mut().set_parameter(2, &s(""));
    let mut out = Vec::new();
    assert!(g.convert_command_to_string(0x2BB4_0833, Some(&p), &mut out));
    assert_eq!(out, b"SetDebugLevel \"a b\" 7 ");
    assert!(g.convert_command_to_string(0x2BB4_0833, None, &mut out));
    assert_eq!(out, b"SetDebugLevel");
    assert!(g.convert_command_to_string(0x30, None, &mut out));
    assert_eq!(out, b"\"My Cmd\"");
    out = b"old".to_vec();
    assert!(!g.convert_command_to_string(0x99, None, &mut out));
    assert!(out.is_empty());
    let q = CommandParameterSet::new_ref();
    q.borrow_mut().set_parameter(0, &s("x y"));
    q.borrow_mut().set_parameter(1, &Variant::Scalar(Elem::U32(3)));
    q.borrow_mut().set_status_parameter_value(0);
    assert!(g.convert_command_output_to_string(Some(&q), &mut out));
    assert_eq!(out, b"\"x y\" 3 0");
    assert!(g.convert_command_output_to_string(Some(&CommandParameterSet::new_ref()), &mut out));
    assert!(out.is_empty());
    assert_eq!(quote_argument(b""), b"");
    assert_eq!(quote_argument(b"a\"b c"), b"\"a\"b c\"");
}

type Handler = Box<dyn Fn(&Rc<CommandMessage>)>;

/// A scripted message server: (type, handler) subscriptions, type 0 = wildcard, and a log.
#[derive(Default)]
struct MsgServer {
    targets: Vec<(u32, Handler)>,
    sent: RefCell<Vec<(u32, Rc<CommandMessage>)>>,
}
impl CommandMessageServer for MsgServer {
    fn message_send(&self, msg: &Rc<CommandMessage>) -> bool {
        self.sent.borrow_mut().push((msg.get_type(), msg.clone()));
        for (ty, h) in self
            .targets
            .iter()
            .filter(|t| t.0 == msg.get_type())
            .chain(self.targets.iter().filter(|t| t.0 == 0))
        {
            let _ = ty;
            h(msg);
        }
        true
    }
}

fn dispatcher(ms: &Rc<MsgServer>) -> Message2CommandDispatcher {
    let mut d = Message2CommandDispatcher::new(conv());
    d.init(Some(ms.clone() as Rc<dyn CommandMessageServer>), None);
    d
}

#[test]
fn t13_dispatch() {
    let mut before = Message2CommandDispatcher::new(conv());
    assert_eq!(before.execute_command(0x1234, None, None), 2);
    let status = Rc::new(RefCell::new(None::<Variant>));
    let st = status.clone();
    let wild = Rc::new(RefCell::new(0u32));
    let w = wild.clone();
    let ms = Rc::new(MsgServer {
        targets: vec![
            (
                0x1234,
                Box::new(move |m: &Rc<CommandMessage>| {
                    if let Some(v) = st.borrow().clone() {
                        m.get_data2()
                            .unwrap()
                            .borrow_mut()
                            .set_parameter(STATUS_PARAMETER_ID, &v);
                    }
                }),
            ),
            (0, Box::new(move |_m: &Rc<CommandMessage>| *w.borrow_mut() += 1)),
        ],
        sent: RefCell::default(),
    });
    let mut d = dispatcher(&ms);
    d.default_input().unwrap().borrow_mut().set_parameter(3, &s("stale"));
    assert_eq!(d.execute_command(0x1234, None, None), 0);
    {
        let sent = ms.sent.borrow();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, 0x1234);
        let m = &sent[0].1;
        assert!(Rc::ptr_eq(&m.get_data1().unwrap(), d.default_input().unwrap()));
        assert_eq!(d.default_input().unwrap().borrow().get_parameter_count(), 0);
        assert!(Rc::ptr_eq(&m.get_data2().unwrap(), d.default_output().unwrap()));
    }
    assert_eq!(
        d.default_output().unwrap().borrow().get_parameter(STATUS_PARAMETER_ID),
        Some(&i32v(0))
    );
    *status.borrow_mut() = Some(i32v(2));
    assert_eq!(d.execute_command(0x1234, None, None), 2);
    *status.borrow_mut() = Some(s("7"));
    assert_eq!(d.execute_command(0x1234, None, None), 7);
    *status.borrow_mut() = None;
    let (i, o) = (CommandParameterSet::new_owned(), CommandParameterSet::new_owned());
    i.borrow_mut().set_parameter(0, &s("a"));
    assert_eq!(d.execute_command(0x1234, Some(&i), Some(&o)), 0);
    {
        let sent = ms.sent.borrow();
        let m = &sent.last().unwrap().1;
        assert!(Rc::ptr_eq(&m.get_data1().unwrap(), &i));
        assert!(Rc::ptr_eq(&m.get_data2().unwrap(), &o));
        assert_eq!(i.borrow().ref_count(), 2); // the caller's and the message's
    }
    assert_eq!(i.borrow().get_parameter_count(), 1);
    assert_eq!(o.borrow().ids(), vec![STATUS_PARAMETER_ID]);
    assert_eq!(d.execute_command(0x5555, None, None), 0);
    assert_eq!(*wild.borrow(), 5);
    let sent = ms.sent.borrow();
    assert!(Rc::ptr_eq(&sent[0].1, &sent[4].1));
}

#[test]
fn t14_end_to_end() {
    let seen = Rc::new(RefCell::new(Vec::<Vec<u32>>::new()));
    let sn = seen.clone();
    let ms = Rc::new(MsgServer {
        targets: vec![(
            0x2BB4_0833,
            Box::new(move |m: &Rc<CommandMessage>| {
                let i = m.get_data1().unwrap();
                assert_eq!(i.borrow().get_parameter(0), Some(&s("5")));
                sn.borrow_mut().push(i.borrow().ids());
                m.get_data2().unwrap().borrow_mut().set_status_parameter_value(3);
            }),
        )],
        sent: RefCell::default(),
    });
    let srv = server();
    *srv.dispatcher.borrow_mut() = Some(dispatcher(&ms));
    let mut g = generator(&srv);
    assert_eq!(g.execute_command(b"SetDebugLevel 5", u32::MAX, None), 3);
    assert_eq!(seen.borrow().len(), 1);
    assert_eq!(g.execute_command(b"Nope", u32::MAX, None), 2);
    assert_eq!(ms.sent.borrow().len(), 1);
}

#[test]
fn t15_message_identity() {
    let m = CommandMessage::new();
    m.add_ref();
    assert_eq!(m.query_interface(0x4B99_446A), Some(MessageFace::Command));
    assert_eq!(m.query_interface(0x6529_7976), Some(MessageFace::Standard));
    assert_eq!(m.query_interface(0x1234_5678), None);
    assert_eq!(m.ref_count(), 3);
    let c = m.create();
    assert_eq!(c.msg_type, m.get_type());
    assert_eq!(c.query_interface(0x4B99_446A), None);
    assert_eq!(c.query_interface(0x6529_7976), Some(MessageFace::Standard));
}

#[test]
fn t16_reference_counts() {
    let mut p = CommandParameterSet::new();
    assert_eq!(p.ref_count(), 0);
    assert_eq!(p.add_ref(), 1);
    assert_eq!(p.release(), 0);
    assert!(p.is_destroyed());
    let mut g = StringCommandGenerator::new(conv());
    assert_eq!(g.ref_count(), 0);
    assert_eq!(g.add_ref(), 1);
    assert_eq!(g.release(), 0);
    assert!(g.is_destroyed());
    let mut d = Message2CommandDispatcher::new(conv());
    assert_eq!(d.add_ref(), 1);
    assert_eq!(d.add_ref(), 2);
    assert_eq!(d.release(), 1);
    assert_eq!(d.release(), 0);
    assert!(d.is_destroyed());
    assert_eq!(CommandParameterSet::new_owned().borrow().ref_count(), 1);
    let mut q = CommandParameterSet::new();
    assert!(q.query_interface(0x4B8C_39E5));
    assert!(!q.query_interface(0x1234));
    assert_eq!(q.ref_count(), 1);
}

#[test]
fn init_shutdown_lifecycle() {
    let ms = Rc::new(MsgServer::default());
    let mut d = dispatcher(&ms);
    let m1 = d.message().unwrap().clone();
    d.init(None, None); // a second Init changes nothing
    assert!(Rc::ptr_eq(&m1, d.message().unwrap()));
    assert!(d.generator().is_some());
    d.shutdown();
    assert!(d.message().is_none() && d.generator().is_none());
    assert_eq!(d.execute_command(1, None, None), 2);
    d.init(Some(ms.clone() as Rc<dyn CommandMessageServer>), None);
    assert!(!Rc::ptr_eq(&m1, d.message().unwrap()));
    assert_eq!(d.execute_command(1, None, None), 0);
    let srv = server();
    let mut g = generator(&srv);
    assert!(g.unused_set().is_some());
    g.shutdown();
    assert!(g.unused_set().is_none() && !g.has_server());
    g.init(Some(srv.clone() as Rc<dyn CommandServer>));
    assert!(g.unused_set().is_none() && g.has_server());
}

#[test]
fn make_lower_rules() {
    assert_eq!(make_lower(b"AbC"), b"abc");
    assert_eq!(make_lower("ÉCOLE".as_bytes()), "école".as_bytes());
    assert_eq!(make_lower(b"\xC9A"), b"\xC9a");
}

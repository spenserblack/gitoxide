mod stats {
    mod from_bytes {
        use gix_filter::eol;

        #[test]
        fn all() {
            let stats = eol::Stats::from_bytes(b"\n\r\nhi\rho\0\tanother line\nother\r\nmixed");
            assert_eq!(
                stats,
                eol::Stats {
                    null: 1,
                    lone_cr: 1,
                    lone_lf: 2,
                    crlf: 2,
                    printable: 27,
                    non_printable: 1,
                }
            );
            assert!(stats.is_binary());
        }
    }
}

mod convert_to_git {
    use bstr::{ByteSlice, ByteVec};
    use gix_filter::eol;
    use gix_filter::eol::AttributesDigest;

    #[test]
    fn with_binary_attribute_is_never_converted() {
        let mut buf = Vec::new();
        let changed = eol::convert_to_git(b"hi\r\nho", AttributesDigest::Binary, &mut buf, no_call).expect("no error");
        assert!(!changed, "the user marked it as binary so it's never being touched");
    }

    #[test]
    fn no_crlf_means_no_work() -> crate::Result {
        let mut buf = Vec::new();
        let changed = eol::convert_to_git(b"hi", AttributesDigest::TextCrlf, &mut buf, no_call).expect("no error");
        assert!(!changed);

        let changed =
            eol::convert_to_git(b"hi", AttributesDigest::TextAutoCrlf, &mut buf, no_object_in_index).expect("no error");
        assert!(!changed, "in auto-mode, the object is queried in the index as well.");
        Ok(())
    }

    #[test]
    fn detected_as_binary() -> crate::Result {
        let mut buf = Vec::new();
        let changed = eol::convert_to_git(
            b"hi\0zero makes it binary",
            AttributesDigest::TextAuto,
            &mut buf,
            no_call,
        )
        .expect("no error");
        assert!(
            !changed,
            "in auto-mode, we have a heuristic to see if the buffer is binary"
        );
        Ok(())
    }

    #[test]
    fn fast_conversion_by_stripping_cr() -> crate::Result {
        let mut buf = Vec::new();
        let changed =
            eol::convert_to_git(b"a\r\nb\r\nc", AttributesDigest::TextCrlf, &mut buf, no_call).expect("no error");
        assert!(changed);
        assert_eq!(buf.as_bstr(), "a\nb\nc", "here carriage returns can just be stripped");
        Ok(())
    }

    #[test]
    fn slower_conversion_due_to_lone_cr() -> crate::Result {
        let mut buf = Vec::new();
        let changed =
            eol::convert_to_git(b"\r\ra\r\nb\r\nc", AttributesDigest::TextCrlf, &mut buf, no_call).expect("no error");
        assert!(changed);
        assert_eq!(
            buf.as_bstr(),
            "\r\ra\nb\nc",
            "here carriage returns cannot be stripped but must be handled in pairs"
        );
        Ok(())
    }

    #[test]
    fn crlf_in_index_prevents_conversion_to_lf() -> crate::Result {
        let mut buf = Vec::new();
        let mut called = false;
        let changed = eol::convert_to_git(b"elligible\n", AttributesDigest::TextAutoInput, &mut buf, |buf| {
            called = true;
            buf.clear();
            buf.push_str("with CRLF\r\n");
            Ok::<_, std::convert::Infallible>(Some(()))
        })
        .expect("no error");
        assert!(called, "in auto mode, the index is queried as well");
        assert!(
            !changed,
            "we saw the CRLF is present in the index, so it's unsafe to make changes"
        );
        Ok(())
    }

    #[allow(clippy::ptr_arg)]
    fn no_call(_buf: &mut Vec<u8>) -> std::io::Result<Option<()>> {
        unreachable!("index function will not be called")
    }

    #[allow(clippy::ptr_arg)]
    fn no_object_in_index(_buf: &mut Vec<u8>) -> std::io::Result<Option<()>> {
        Ok(Some(()))
    }
}

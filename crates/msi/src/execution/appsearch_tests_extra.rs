    #[test]
    fn test_appsearch_extra_coverage() {
        let mut db = LinkedDatabase::new().unwrap();
        // 1) AppSearch empty prop/sig
        db.add_record("AppSearch", Record::with_fields(vec![FieldValue::String("".to_string()), FieldValue::String("SigEmpty".to_string())]));
        db.add_record("AppSearch", Record::with_fields(vec![FieldValue::String("PROP".to_string()), FieldValue::String("".to_string())]));
        
        // 2) Unresolved signature
        db.add_record("AppSearch", Record::with_fields(vec![FieldValue::String("PROP_UNRES".to_string()), FieldValue::String("SigUnresolved".to_string())]));

        // 3) IniLocator with locator_type=0 (file) and missing signature (line 196-199, 244)
        db.add_record("AppSearch", Record::with_fields(vec![FieldValue::String("MYINIFILE".to_string()), FieldValue::String("SigIniFile".to_string())]));
        db.add_record("IniLocator", Record::with_fields(vec![
            FieldValue::String("SigIniFile".to_string()),
            FieldValue::String("test2.ini".to_string()),
            FieldValue::String("Sec2".to_string()),
            FieldValue::String("Key2".to_string()),
            FieldValue::Short(1), // field using FieldValue::Short! covers get_i32 Short
            FieldValue::Long(0), // file type
        ]));

        // 4) RegLocator where check_signature fails
        db.add_record("AppSearch", Record::with_fields(vec![FieldValue::String("MYREGFAIL".to_string()), FieldValue::String("SigRegFail".to_string())]));
        db.add_record("RegLocator", Record::with_fields(vec![
            FieldValue::String("SigRegFail".to_string()),
            FieldValue::Long(2),
            FieldValue::String("Software\\Fail".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
        ]));
        db.add_record("Signature", Record::with_fields(vec![
            FieldValue::String("SigRegFail".to_string()),
            FieldValue::String("fail.exe".to_string()),
            FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null
        ]));

        // 5) DrLocator missing path but has parent (line 216), parent resolves to "C:\Parent"
        db.add_record("AppSearch", Record::with_fields(vec![FieldValue::String("MYDIRPARENT".to_string()), FieldValue::String("SigDirParent".to_string())]));
        db.add_record("DrLocator", Record::with_fields(vec![
            FieldValue::String("SigDirParent".to_string()),
            FieldValue::String("SigParentReg".to_string()), // parent
            FieldValue::Null, // no path
            FieldValue::Null,
        ]));
        // Provide parent via RegLocator
        db.add_record("RegLocator", Record::with_fields(vec![
            FieldValue::String("SigParentReg".to_string()),
            FieldValue::Long(2),
            FieldValue::String("Software\\Parent".to_string()),
            FieldValue::Null,
            FieldValue::Long(2), // raw type
        ]));

        // 6) DrLocator with parent and path (line 213)
        db.add_record("AppSearch", Record::with_fields(vec![FieldValue::String("MYDIRBOTH".to_string()), FieldValue::String("SigDirBoth".to_string())]));
        db.add_record("DrLocator", Record::with_fields(vec![
            FieldValue::String("SigDirBoth".to_string()),
            FieldValue::String("SigParentReg".to_string()), // parent
            FieldValue::String("Child".to_string()), // path
            FieldValue::Null,
        ]));

        // 7) DrLocator find_dir fails (Line 390)
        db.add_record("AppSearch", Record::with_fields(vec![FieldValue::String("MYDIRFAIL".to_string()), FieldValue::String("SigDirFail".to_string())]));
        db.add_record("DrLocator", Record::with_fields(vec![
            FieldValue::String("SigDirFail".to_string()),
            FieldValue::Null,
            FieldValue::String("C:\\MissingDir".to_string()),
            FieldValue::Null,
        ]));

        // 8) DrLocator find_file empty path (Line 376) and fails (Line 383, 225)
        db.add_record("AppSearch", Record::with_fields(vec![FieldValue::String("MYFILEFAIL".to_string()), FieldValue::String("SigFileFail".to_string())]));
        db.add_record("DrLocator", Record::with_fields(vec![
            FieldValue::String("SigFileFail".to_string()),
            FieldValue::Null,
            FieldValue::String("".to_string()), // empty path
            FieldValue::Null,
        ]));
        db.add_record("Signature", Record::with_fields(vec![
            FieldValue::String("SigFileFail".to_string()),
            FieldValue::String("missing.exe".to_string()),
            FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null
        ]));

        // 9) DrLocator check_signature fails (Line 224)
        db.add_record("AppSearch", Record::with_fields(vec![FieldValue::String("MYFILEFAIL2".to_string()), FieldValue::String("SigFileFail2".to_string())]));
        db.add_record("DrLocator", Record::with_fields(vec![
            FieldValue::String("SigFileFail2".to_string()),
            FieldValue::Null,
            FieldValue::String("C:\\Found".to_string()), 
            FieldValue::Null,
        ]));
        db.add_record("Signature", Record::with_fields(vec![
            FieldValue::String("SigFileFail2".to_string()),
            FieldValue::String("found_bad_sig.exe".to_string()),
            FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null, FieldValue::Null
        ]));

        let mut host = MockHost::default();
        host.ini.insert(
            ("test2.ini".to_string(), "Sec2".to_string(), "Key2".to_string(), Some(1)),
            "C:\\IniFile".to_string()
        );
        host.registry.insert(
            (2, "Software\\Fail".to_string(), None),
            "C:\\FailFile.exe".to_string()
        );
        host.signatures.insert("C:\\FailFile.exe".to_string(), false); // signature check fails

        host.registry.insert(
            (2, "Software\\Parent".to_string(), None),
            "C:\\Parent".to_string()
        );
        host.dirs.insert("C:\\Parent".to_string(), "C:\\Parent".to_string());
        host.dirs.insert("C:\\Parent\\Child".to_string(), "C:\\Parent\\Child".to_string());

        // For 9
        host.files.insert("C:\\Found\\found_bad_sig.exe".to_string(), "C:\\Found\\found_bad_sig.exe".to_string());
        host.signatures.insert("C:\\Found\\found_bad_sig.exe".to_string(), false);

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().unwrap();
        
        assert_eq!(res.get("MYINIFILE").unwrap(), "C:\\IniFile");
        assert_eq!(res.get("MYDIRPARENT").unwrap(), "C:\\Parent");
        assert_eq!(res.get("MYDIRBOTH").unwrap(), "C:\\Parent\\Child");
        
        // These should not be in the results
        assert!(!res.contains_key("PROP_UNRES"));
        assert!(!res.contains_key("MYREGFAIL"));
        assert!(!res.contains_key("MYDIRFAIL"));
        assert!(!res.contains_key("MYFILEFAIL"));
        assert!(!res.contains_key("MYFILEFAIL2"));
        assert!(!res.contains_key("PROP"));
    }

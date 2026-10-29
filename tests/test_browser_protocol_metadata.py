"""Only protocol identity and permitted numeric metadata may leave the parser."""
import json
import unittest
from browser_fixture import EXPECTED, ime_metadata, keyboard_focus_metadata, text_input_metadata


class BrowserProtocolMetadataTests(unittest.TestCase):
    def test_strings_and_cursor_payloads_are_omitted(self):
        for method,args in [('commit_string','"synthetic private value"'),
                            ('set_surrounding_text','"synthetic private value", 23, 23'),
                            ('preedit_string','"synthetic private value", 2, 3'),
                            ('set_cursor_rectangle','1, 2, 3, 4')]:
            result=text_input_metadata('[1489909.358] {Default Queue} zwp_text_input_v3#20.'+method+'('+args+')')
            self.assertEqual(set(result),{'stamp','direction','object','method'})
            self.assertNotIn('synthetic',json.dumps(result))

    def test_direction_serial_and_content_type_with_both_log_formats(self):
        result=text_input_metadata('[ 1.25] {Default Queue}  -> zwp_text_input_v3#20.commit(42)')
        self.assertEqual((result['direction'],result['numbers']),('request',[42]))
        result=text_input_metadata('[ 2.5] zwp_text_input_v3@20.done(0)')
        self.assertEqual((result['direction'],result['numbers']),('event',[0]))
        result=text_input_metadata('[ 3] -> zwp_text_input_v3#20.set_content_type(192, 8)')
        self.assertEqual(result['numbers'],[192,8])

    def test_other_protocols_and_non_protocol_stderr_are_ignored(self):
        self.assertIsNone(text_input_metadata('[ 4] -> wl_data_source#3.offer("private data")'))
        self.assertIsNone(text_input_metadata('arbitrary stderr content'))

    def test_focus_metadata_keeps_surface_identity_and_omits_keys(self):
        result=keyboard_focus_metadata('[ 4] {Default Queue} wl_keyboard#7.enter(19, wl_surface#36, array[8])')
        self.assertEqual(result['surface'],'wl_surface#36')
        self.assertEqual(set(result),{'stamp','direction','object','method','surface'})
        self.assertIsNone(keyboard_focus_metadata('[ 5] wl_keyboard#7.key(20, 900, 30, 1)'))

    def test_synthetic_diagnostic_states_preserve_canonical_indices(self):
        self.assertEqual(EXPECTED.index('demo42'),20)
        self.assertEqual(len(EXPECTED),len(set(EXPECTED)))
        for value in ['o42','dem42','demorw','tarotxzdem']:
            self.assertIn(value,EXPECTED)

    def test_ime_metadata_discards_commit_selection_and_key_payloads(self):
        for method,args in [
            ('OnCommitCompositionNative','aContext=0x123), commitString="synthetic private value"'),
            ('DispatchCompositionCommitEvent','aContext=0x123, aCommitString=0x456, ("synthetic private value")'),
            ('OnSelectionChange','aCaller=0x123, aIMENotification={ synthetic private value }'),
            ('MaybeDispatchKeyEventAsProcessedByIME','aFollowingEvent=eContentCommandInsertText), dispatch keyval=synthetic private value'),
        ]:
            result=ime_metadata('[Parent 42: Main Thread]: I/IMEHandler 0x123 '+method+'('+args+')\n')
            self.assertEqual(result,{'process':'Parent','pid':42,'level':'I','method':method,'status':'call'})
            self.assertNotIn('synthetic',json.dumps(result))

    def test_ime_metadata_reasons_require_exact_fixed_messages(self):
        prefix='[Parent 42: Main Thread]: E/IMEHandler 0x123   DispatchCompositionChangeEvent(), '
        result=ime_metadata(prefix+'FAILED, inserting text failed\n')
        self.assertEqual((result['status'],result['reason']),('failed','insert-text-failed'))
        result=ime_metadata(prefix+'FAILED, inserting text failed synthetic private value\n')
        self.assertEqual(result['status'],'failed');self.assertNotIn('reason',result)
        self.assertNotIn('synthetic',json.dumps(result))
        self.assertIsNone(ime_metadata(prefix.replace('IMEHandler','OtherModule')+'FAILED, inserting text failed'))
        self.assertIsNone(ime_metadata(prefix.replace('DispatchCompositionChangeEvent','UnlistedMethod')+'private value'))


if __name__=='__main__':unittest.main()

"""Only protocol identity and permitted numeric metadata may leave the parser."""
import json
import unittest
from browser_fixture import text_input_metadata


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


if __name__=='__main__':unittest.main()

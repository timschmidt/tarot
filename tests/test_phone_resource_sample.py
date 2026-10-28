"""Accounting regressions: PID reuse, suspend clocks and DRM aliases."""
import unittest

from phone_resource_sample import cpu_summary, deduplicate_gpu, effective_uid, gpu_client, memory, proc_stat


class ResourceAccounting(unittest.TestCase):
    def test_stat_handles_spaces_and_parentheses_in_comm(self):
        fields = ['S'] + ['0']*21
        fields[11], fields[12], fields[19] = '300', '17', '123456'
        parsed = proc_stat('12 (a tricky ) name) '+' '.join(fields))
        self.assertEqual(parsed, {'cpu_ticks': 317, 'start': 123456})
        self.assertEqual(memory('Rss: 10 kB\nPss: 7 kB\nSwapPss: 2 kB'),
                         {'Rss_bytes': 10240, 'Pss_bytes': 7168, 'SwapPss_bytes': 2048})
        with self.assertRaises(ValueError):
            memory('Rss: 10 kB')
        self.assertEqual(effective_uid('Name: tarot-lock\nUid:\t10000\t10000\t10000\t10000\n'), 10000)
        self.assertEqual(effective_uid('Uid:\t10000\t0\t0\t0\n'), 0)

    def test_gpu_aliases_are_not_summed_and_device_ids_are_scoped(self):
        def info(size, device):
            return gpu_client(f'drm-driver: panfrost\ndrm-client-id: 128\ndrm-total-memory: {size} KiB\ndrm-curfreq-fragment: 200000000 Hz', device)
        result = deduplicate_gpu([info(100, 'gpu0'), info(110, 'gpu0'), info(30, 'gpu1')])
        self.assertEqual(len(result), 2)
        self.assertEqual(result[0]['descriptors'], 2)
        self.assertEqual(result[0]['counters']['drm-total-memory'], {'value': 110*1024, 'unit': 'bytes'})
        self.assertEqual(result[1]['counters']['drm-total-memory']['value'], 30*1024)
        self.assertIsNone(gpu_client('pos: 0\nflags: 2', 'gpu0'))

    def test_cpu_budget_is_unavailable_on_pid_reuse_membership_change_or_sleep(self):
        def sample(t, ticks, start=1, boot=None):
            return {'clocks': {'monotonic': t, 'boottime': t if boot is None else boot},
                    'members': [{'pid': 42, 'start': start, 'exe': '/usr/bin/tarot', 'cpu_ticks': ticks}],
                    'raced_pids': []}
        first = sample(0, 100)
        self.assertAlmostEqual(cpu_summary([first, sample(300, 250)], 100)['percent_of_one_core'], .5)
        for final in [sample(300, 250, start=2), sample(300, 250, boot=315)]:
            self.assertIsNone(cpu_summary([first, final], 100)['percent_of_one_core'])
        middle = sample(150, 180)
        middle['members'].append({'pid': 43, 'start': 2, 'exe': '/usr/bin/tarot-lock', 'cpu_ticks': 10})
        self.assertIsNone(cpu_summary([first, middle, sample(300, 250)], 100)['percent_of_one_core'])


if __name__ == '__main__':
    unittest.main()

"""Regression cases for real out-of-order Firefox loopback reports."""
import unittest
from browser_observations import latest_reports


def report(state,time,sequence,document='first',page='main'):
    return {'event':'input','page':page,'document':document,'observed':time,'sequence':sequence,
        'fields':{'primary':{'state':state,'length':0}}}


class BrowserObservationTests(unittest.TestCase):
    def test_late_resize_cannot_overwrite_newer_input(self):
        events=[report(5,10,1),report(6,12,3),report(5,11,2)]
        self.assertEqual(latest_reports(events)['main']['fields']['primary']['state'],6)
        # A genuinely newer unexpected value still fails the exact-state gate.
        events.append(report(-1,13,4))
        self.assertEqual(latest_reports(events)['main']['fields']['primary']['state'],-1)

    def test_navigation_and_restored_documents_use_observation_time(self):
        events=[report(5,10,50),report(0,20,1,'second'),report(6,11,51)]
        self.assertEqual(latest_reports(events)['main']['document'],'second')
        events.append(report(6,30,52,'first'))
        self.assertEqual(latest_reports(events)['main']['document'],'first')

    def test_coarse_clock_ties_use_sequence_and_pages_remain_independent(self):
        events=[report(6,10,3),report(5,10,2),report(24,11,1,'popup','popup')]
        latest=latest_reports(events)
        self.assertEqual(latest['main']['fields']['primary']['state'],6)
        self.assertEqual(latest['popup']['fields']['primary']['state'],24)


if __name__=='__main__':unittest.main()

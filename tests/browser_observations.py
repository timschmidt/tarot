"""Order synthetic DOM snapshots by browser observation, not HTTP arrival."""


def latest_reports(events):
    latest={}
    for event in events:
        if 'fields' not in event:continue
        assert isinstance(event['document'],str) and event['document']
        assert type(event['sequence']) is int and event['sequence']>0
        key=(event['observed'],event['sequence'])
        previous=latest.get(event['page'])
        if previous is None or key>(previous['observed'],previous['sequence']):latest[event['page']]=event
    return latest

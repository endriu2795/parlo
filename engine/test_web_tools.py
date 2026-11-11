import io
import json
import unittest
from urllib.request import Request
from unittest.mock import patch

import web_tools as web


class Reply(io.BytesIO):
    status = 200


class Opener:
    def __init__(self, html, status=200):
        self.html, self.status, self.calls = html, status, 0

    def open(self, req, timeout):
        self.calls += 1
        self.request, self.timeout = req, timeout
        reply = Reply(self.html)
        reply.status = self.status
        return reply


class SearchTests(unittest.TestCase):
    def test_results_decode_redirects_deduplicate_and_bound_output(self):
        html = '''<a class="result-link" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.org%2Fitaliano">Città &amp; voce</a>
        <td class="result-snippet">Una <b>fonte</b> italiana.</td>
        <a class="result-link" href="https://example.org/italiano">Duplicate</a>
        <a class="result-link" href="javascript:bad()">Bad</a>
        <a class="result-link" href="https://example.org/due">Due</a>'''
        opener = Opener(html.encode())
        value = web.search({'query': 'voce italiana', 'max_results': 1}, opener)
        self.assertEqual(value['results'], [{'title': 'Città & voce', 'url': 'https://example.org/italiano', 'content': 'Una fonte italiana.'}])
        self.assertEqual(opener.calls, 1)
        self.assertEqual(opener.timeout, 15)
        self.assertTrue(opener.request.full_url.startswith('https://lite.duckduckgo.com/lite/?q='))
        value = web.search({'query': 'prova'}, Opener(html.encode()))
        self.assertEqual(len(value['results']), 2)

    def test_bad_arguments_never_start_network(self):
        opener = Opener(b'')
        for args in [{}, {'query': ''}, {'query': 'a\nb'}, {'query': 'a'*2049},
                     {'query': 'a', 'max_results': True}, {'query': 'a', 'max_results': 11},
                     {'query': 'a', 'url': 'http://localhost'}, []]:
            with self.subTest(args=str(args)[:80]), self.assertRaises(web.SearchError):
                web.search(args, opener)
        self.assertEqual(opener.calls, 0)

    def test_challenges_limits_and_oversized_responses_stop_without_retry(self):
        for html, status in [(b'<form id="challenge-form">', 200), (b'', 202), (b'x'*(web.MAX_HTML+1), 200)]:
            opener = Opener(html, status)
            with self.assertRaises(web.SearchError):
                web.search({'query': 'prova'}, opener)
            self.assertEqual(opener.calls, 1)

    def test_redirects_cannot_fetch_other_hosts_or_plain_http(self):
        handler = web.SearchRedirects()
        req = Request('https://lite.duckduckgo.com/lite/?q=prova')
        for url in ['https://example.org/', 'http://duckduckgo.com/', 'http://127.0.0.1/', 'file:///secret']:
            with self.assertRaises(web.SearchError):
                handler.redirect_request(req, None, 302, '', {}, url)
        self.assertIsNotNone(handler.redirect_request(req, None, 302, '', {}, 'https://duckduckgo.com/lite/'))

    def test_protocol_schema_and_sanitized_errors(self):
        tools = web.response({'id': 1, 'method': 'tools/list'})['result']['tools']
        self.assertEqual(tools[0]['name'], 'WebSearch')
        self.assertEqual(tools[0]['inputSchema']['required'], ['query'])
        self.assertIsNone(web.response({'method': 'notifications/initialized'}))
        with patch.object(web, 'search', side_effect=RuntimeError('private-response-body')):
            result = web.response({'id': 2, 'method': 'tools/call', 'params': {'name': 'WebSearch', 'arguments': {'query': 'prova'}}})
        self.assertTrue(result['result']['isError'])
        self.assertNotIn('private-response-body', json.dumps(result))
        self.assertTrue(result['result']['content'][0]['text'].startswith('WebSearch: '))


if __name__ == '__main__':
    unittest.main()

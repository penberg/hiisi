#!/usr/bin/env python3
import pytest
import requests
import json


def test_create_namespace():
    url = 'http://127.0.0.1:8081/v1/namespaces/foo/create'
    headers = {'Content-Type': 'application/json'}
    data = {}
    resp = requests.post(url, headers=headers, data=json.dumps(data))
    try:
        assert resp.status_code == 200, f"Unexpected status code: {resp.status_code}. Response content: {resp.text}"
    except AssertionError as e:
        print(f"Request URL: {url}")
        print(f"Response headers: {resp.headers}")
        raise e

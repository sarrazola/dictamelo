#!/usr/bin/env python3
"""Live smoke test for the Dictamelo free backend (macOS, requests, authenticated Supabase CLI).
Creates and removes only its own example.invalid account. Does not send an email.
Uses a short synthesized recording and therefore makes one real provider request.
"""
import sys
if sys.argv[1:] != ["--live"]:
    raise SystemExit("Usage: python3 scripts/verify-free-backend.py --live")
import json,subprocess,uuid,requests,tempfile
from pathlib import Path
BASE='https://iburiyhhfodndqgmsaot.supabase.co'
keys=json.loads(subprocess.run(['supabase','projects','api-keys','--project-ref','iburiyhhfodndqgmsaot','-o','json'],capture_output=True,text=True,check=True).stdout)
service=next(k['api_key'] for k in keys if k['name']=='service_role')
anon=next(k['api_key'] for k in keys if k['name']=='anon')
admin=requests.Session();admin.headers.update(apikey=service,Authorization='Bearer '+service)
email='dictamelo-test-'+str(uuid.uuid4())+'@example.invalid'
uid=None
audio_dir=tempfile.TemporaryDirectory(prefix='dictamelo-live-audio-')
aiff=str(Path(audio_dir.name)/'test.aiff')
wav_path=str(Path(audio_dir.name)/'test.wav')
try:
 r=admin.post(BASE+'/auth/v1/admin/generate_link',json={'type':'magiclink','email':email},timeout=30);r.raise_for_status();data=r.json();uid=data.get('user',data)['id']
 token=data.get('email_otp') or data.get('properties',{}).get('email_otp');assert token
 r=requests.post(BASE+'/auth/v1/verify',headers={'apikey':anon},json={'type':'email','email':email,'token':token},timeout=30);r.raise_for_status();session=r.json();access=session['access_token']
 client=requests.Session();client.headers['Authorization']='Bearer '+access
 r=client.post(BASE+'/functions/v1/usage',json={},timeout=30);assert r.status_code==200,r.text;assert r.json()['usedWords']==0
 print('PASS: email code verified; authenticated account starts at zero')
 subprocess.run(['say','-o',aiff,'Hello. This is a test of the free weekly word allowance.'],check=True)
 subprocess.run(['afconvert','-f','WAVE','-d','LEI16@16000','-c','1',aiff,wav_path],check=True)
 wav=Path(wav_path).read_bytes()
 r=client.post(BASE+'/functions/v1/transcribe',files={'file':('test.wav',wav,'audio/wav')},timeout=120);assert r.status_code==200,r.text
 transcript=r.json()['text'];assert len(transcript)>10
 r=client.post(BASE+'/functions/v1/usage',json={},timeout=30);r.raise_for_status();used=r.json()['usedWords'];assert used>0
 print('PASS: live transcription through free backend; words recorded:',used)
 r=admin.patch(BASE+'/rest/v1/free_weekly_usage',params={'user_id':'eq.'+uid},json={'words':2000},timeout=30);r.raise_for_status()
 r=client.post(BASE+'/functions/v1/transcribe',files={'file':('test.wav',wav,'audio/wav')},timeout=30);assert r.status_code==429,r.text
 print('PASS: exhausted allowance returns HTTP 429')
 r=requests.post(BASE+'/functions/v1/usage',json={},timeout=30);assert r.status_code==401
 r=requests.post(BASE+'/functions/v1/transcribe',headers={'Authorization':'Bearer invalid'},files={'file':('test.wav',wav,'audio/wav')},timeout=30);assert r.status_code==401
 r=requests.post(BASE+'/rest/v1/rpc/free_usage',headers={'apikey':anon,'Authorization':'Bearer '+access},json={'p_user':uid},timeout=30);assert r.status_code in (401,403,404)
 print('PASS: missing/invalid auth rejected; direct client access to quota blocked')
 r=requests.post(BASE+'/auth/v1/token?grant_type=refresh_token',headers={'apikey':anon},json={'refresh_token':session['refresh_token']},timeout=30);r.raise_for_status()
 print('PASS: session refresh')
finally:
 audio_dir.cleanup()
 if uid:
  r=admin.delete(BASE+'/auth/v1/admin/users/'+uid,timeout=30);r.raise_for_status();print('Removed only the temporary test account and its usage')

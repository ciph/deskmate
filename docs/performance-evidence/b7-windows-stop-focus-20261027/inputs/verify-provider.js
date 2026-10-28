(async () => {try {const count=await window.__TAURI__.core.invoke('verify_api_key',{providerId:'winb7fixture',baseUrl:'http://127.0.0.1:54861/v1',apiKey:'b7-fixture-not-a-secret',manualModelIds:''});return {ok:true,count};}catch(e){return {ok:false,error:String(e)}}})()


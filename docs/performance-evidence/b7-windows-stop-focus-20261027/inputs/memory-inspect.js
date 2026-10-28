(async()=>({exported:await window.__TAURI__.core.invoke('memory_export'),list:await window.__TAURI__.core.invoke('memory_list',{query:{}})}))()

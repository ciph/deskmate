using System;
using System.IO;
using System.Windows.Forms;
using System.Drawing;
using System.Runtime.InteropServices;
class DragSource {
 [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
 [STAThread] static void Main(string[] args) {
  SetProcessDPIAware(); Application.EnableVisualStyles();
  string root=Path.GetFullPath(args[0]); string log=Path.GetFullPath(args[1]);
  var form=new Form { Text="B7 Synthetic Drag Source",Width=580,Height=540,StartPosition=FormStartPosition.Manual,Location=new Point(1100,120),TopMost=true };
  var label=new Label {Text="Synthetic fixtures only. Hold left mouse and drag to YUME.",Left=20,Top=15,Width=530,Height=40};form.Controls.Add(label);
  string[] names={"B7-video-30s.mp4","B7-audio-10s.mp3","B7-audio-8s.wav","folder","B7-bad-video.mp4"};
  for(int i=0;i<names.Length;i++) {
   string name=names[i], path=Path.Combine(root,name);
   if(!File.Exists(path)&&!Directory.Exists(path))throw new Exception("Missing fixture "+name);
   var b=new Button {Text="DRAG: "+name,Left=20,Top=65+i*72,Width=520,Height=58};
   b.MouseDown+=(sender,e)=>{if(e.Button!=MouseButtons.Left)return;var data=new DataObject(DataFormats.FileDrop,new string[]{path});
    File.AppendAllText(log,DateTime.UtcNow.ToString("o")+" start "+name+Environment.NewLine);
    var result=b.DoDragDrop(data,DragDropEffects.Copy);
    File.AppendAllText(log,DateTime.UtcNow.ToString("o")+" result "+name+" "+result+Environment.NewLine);
    label.Text="Last: "+name+" => "+result+" (verify target separately)";
   };form.Controls.Add(b);
  }
  Application.Run(form);
 }
}

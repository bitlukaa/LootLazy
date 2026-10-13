using System;
using System.Windows.Forms;
using System.Runtime.InteropServices;

namespace DummyGame
{
    sealed class BackgroundOwner : Form
    {
        protected override void SetVisibleCore(bool value)
        {
            // Never show a separate window, including during startup or restoration.
            base.SetVisibleCore(false);
        }
    }
    sealed class GameSession : Form
    {
        protected override bool ShowWithoutActivation { get { return true; } }
        protected override CreateParams CreateParams
        {
            get { CreateParams parameters = base.CreateParams; parameters.ExStyle &= ~0x00040000; return parameters; }
        }
    }
    static class Program
    {
        [STAThread]
        static void Main(string[] args)
        {
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            
            Form owner = new BackgroundOwner();
            owner.ShowInTaskbar = false;
            IntPtr ownerHandle = owner.Handle;
            bool nativeTest = args.Length >= 3 && args[0] == "--title" && args[2] == "--native-window";
            Form dummyForm = nativeTest ? (Form)new GameSession() : new BackgroundOwner();
            dummyForm.Text = args.Length >= 2 && args[0] == "--title" ? args[1] : "LootLazy";
            dummyForm.Owner = owner;
            dummyForm.ShowInTaskbar = nativeTest;
            dummyForm.WindowState = nativeTest ? FormWindowState.Minimized : FormWindowState.Normal;
            dummyForm.FormBorderStyle = nativeTest ? FormBorderStyle.FixedSingle : FormBorderStyle.None;
            IntPtr sessionHandle = dummyForm.Handle;
            
            System.Windows.Forms.Timer parentMonitor = null;
            if (args.Length >= 5 && args[3] == "--parent")
            {
                int parentId;
                if (int.TryParse(args[4], out parentId))
                {
                    System.Diagnostics.Process parent = null;
                    try { parent = System.Diagnostics.Process.GetProcessById(parentId); } catch { return; }
                    parentMonitor = new System.Windows.Forms.Timer();
                    parentMonitor.Interval = 1000;
                    parentMonitor.Tick += delegate { try { if (!parent.HasExited) return; } catch {} dummyForm.Close(); };
                    parentMonitor.Start();
                }
            }
            Application.Run(dummyForm);
            if (parentMonitor != null) parentMonitor.Dispose();
            owner.Dispose();
        }
    }
}

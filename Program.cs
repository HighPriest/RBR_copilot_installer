using System.Windows.Forms;

namespace Pacenotes_Installer
{
    internal static class Program
    {
        /// <summary>
        ///  The main entry point for the application.
        /// </summary>
        [STAThread]
        static void Main()
        {
            // To customize application configuration such as set high DPI settings or default font,
            // see https://aka.ms/applicationconfiguration.
            ApplicationConfiguration.Initialize();
            DownloadManager downloadManager = new DownloadManager();
            Application.Run(new InstallationForm(downloadManager));
        }
    }
}
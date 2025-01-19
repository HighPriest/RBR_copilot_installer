using System.Globalization;
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

            // Setup default translation, based on system settings. I have no idea which one works where, so all of them are set.
            Application.CurrentCulture = System.Globalization.CultureInfo.CurrentCulture;
            Thread.CurrentThread.CurrentCulture = System.Globalization.CultureInfo.CurrentCulture;
            Thread.CurrentThread.CurrentUICulture = System.Globalization.CultureInfo.CurrentCulture;

            // Create a modal form to change localization
            LocalizationChangeForm localizationChangeForm = new LocalizationChangeForm(System.Globalization.CultureInfo.CurrentCulture);
            if (localizationChangeForm.ShowDialog() != DialogResult.OK) return;

            // Get the selected culture from the localization change form
            CultureInfo selectedCulture = localizationChangeForm.SelectedCulture;
            Application.CurrentCulture = selectedCulture;
            Thread.CurrentThread.CurrentCulture = selectedCulture;
            Thread.CurrentThread.CurrentUICulture = selectedCulture;

            // Initialize download manager, before opening the installer UI
            DownloadManager downloadManager = new DownloadManager();

            // Open the installer UI
            Application.Run(new InstallationForm(downloadManager));
        }
    }
}
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

            Application.CurrentCulture = CultureInfo.InvariantCulture;

            // Create a modal form to change localization
            LocalizationChangeForm localizationChangeForm = new LocalizationChangeForm();

            if (localizationChangeForm.ShowDialog() != DialogResult.OK)
            {
                return;
            }

            // Get the selected culture from the localization change form
            CultureInfo selectedCulture = localizationChangeForm.SelectedCulture;
            Application.CurrentCulture = selectedCulture;
            Thread.CurrentThread.CurrentCulture = selectedCulture;
            Thread.CurrentThread.CurrentUICulture = selectedCulture;

            DownloadManager downloadManager = new DownloadManager();
            Application.Run(new InstallationForm(downloadManager));
        }
    }
}
using System;
using System.Collections.Generic;
using System.Drawing.Text;
using System.Linq;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using System.Threading.Tasks;

using Pacenotes_Installer.Classes;

namespace Pacenotes_Installer
{
    internal class DownloadManager
    {
        private static Supabase.Client supabase;
        public DownloadManager()
        {
            string PublicURL = "";
            string PublicKey = "";

            InitializeSupabase(PublicURL, PublicKey);
        }

        public static void InitializeSupabase(string url, string key)
        {
            supabase.Realtime.Disconnect();
            var options = new Supabase.SupabaseOptions
            {
                AutoConnectRealtime = true
            };

            supabase = new Supabase.Client(url, key, options);

            try
            {
                Task.Run(() => supabase.InitializeAsync()).Wait();
            }
            catch
            {
                // set status to non-initialized
            }
        }

        #region InstallationMethods
        public void createRBRBackup(string directory)
        {

        }
        public void restoreRBRBackup(string directory)
        {

        }
        public void installFile(string destinationDir, string fileName)
        {

        }
        #endregion InstallationMethods

        #region DownloadMethods
        public void ListAllFiles()
        {

        }

        public void DownloadFile(string fileName) 
        {

        }

        public void ExtractZipFile(byte[] zipFileBytes, string destinationDirectory)
        {

        }
        #endregion DownloadMethods

        #region ConfigurationMethods
        public void CreateRBRConfiguration(string destinationDir)
        {

        }

        public void ReadRBRConfiguration(string directory)
        {

        }

        #endregion ConfigurationMethods
    }
}

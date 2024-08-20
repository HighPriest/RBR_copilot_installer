using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing.Text;
using System.IO.Compression;
using System.Linq;
using System.Security.Cryptography.X509Certificates;
using System.Security.Policy;
using System.Text;
using System.Threading.Tasks;
using Microsoft.IdentityModel.Tokens;
using Pacenotes_Installer.Classes;

namespace Pacenotes_Installer
{
    enum Status : byte
    {
        disconnected,
        connected,
        authorized,
        files_listed,
        downloaded
    }

    internal class DownloadManager
    {
        private static Supabase.Client supabase;
        private string publicURL, publicKey;
        public string privateKey;
        public Status status = 0;

        public List<Supabase.Storage.FileObject> public_files;
        public List<Supabase.Storage.FileObject> private_files;

        public DownloadManager()
        {
            string PublicURL = "";
            string PublicKey = "";

            var options = new Supabase.SupabaseOptions
            {
                AutoConnectRealtime = true
            };

            supabase = new Supabase.Client(publicURL, publicKey, options);
        }

        public void InitializeSupabase()
        {
            try
            {
                Task.Run(() => supabase.InitializeAsync()).Wait();
                status = Status.connected;
            }
            catch
            {
                status = Status.disconnected;
            }
        }

        public void InitializeSupabase(string key)
        {
            var options = new Supabase.SupabaseOptions
            {
                AutoConnectRealtime = true
            };

            privateKey = key;
            Supabase.Client _supabase = new Supabase.Client(publicURL, key, options);

            try
            {
                Task.Run(() => _supabase.InitializeAsync()).Wait();
                if (supabase.Realtime.Socket.IsConnected) supabase.Realtime.Disconnect();
                supabase = _supabase;
                status = Status.authorized;
            }
            catch
            {
                // Throw a box for the user, stating that their code was wrong
            }
        }

        #region InstallationMethods
        public void createRBRBackup(string directory)
        {

        }
        public void restoreRBRBackup(string directory)
        {

        }
        public void installFile(string destinationDir, string fileName, System.ComponentModel.BackgroundWorker backgroundWorker)
        {
            ExtractZipFile(Task.Run(() => DownloadFile(fileName, backgroundWorker)).GetAwaiter().GetResult(), destinationDir);
        }

        public void ExtractZipFile(byte[] zipFileBytes, string destinationDirectory)
        {
            // Convert byte[] to MemoryStream
            using (MemoryStream zipStream = new MemoryStream(zipFileBytes))
            {
                // Create a ZipArchive from the MemoryStream
                using (ZipArchive archive = new ZipArchive(zipStream))
                {
                    // Ensure the destination directory exists
                    Directory.CreateDirectory(destinationDirectory);

                    // Extract each entry to the destination directory
                    foreach (ZipArchiveEntry entry in archive.Entries)
                    {
                        string destinationPath = Path.Combine(destinationDirectory, entry.FullName);

                        // Create the directory if it doesn't exist
                        if (entry.FullName.EndsWith("/"))
                        {
                            Directory.CreateDirectory(destinationPath);
                        }
                        else
                        {
                            // Create the file and extract its contents
                            entry.ExtractToFile(destinationPath, overwrite: true);
                        }
                    }
                }
            }
        }
        #endregion InstallationMethods

        #region DownloadMethods
        public void ListAllFiles()
        {
            if (status == Status.disconnected || !supabase.Realtime.Socket.IsConnected)
            {
                return;
            }

            public_files = new List<Supabase.Storage.FileObject>();

            foreach (Supabase.Storage.FileObject _file in Task.Run(() => supabase.Storage.From("Public").List()).GetAwaiter().GetResult())
            {
                recursiveSupabaseFileListing(public_files, _file, "");
            }

            status = Status.files_listed;
            foreach (var file in public_files)
            {
                Debug.WriteLine("File Name: " + file.Name);
            }
            Debug.WriteLine("FilesListed: " + status);
        }

        public void recursiveSupabaseFileListing(List<Supabase.Storage.FileObject> files_list, Supabase.Storage.FileObject file, string path)
        {
            string _path = path.IsNullOrEmpty() ? file.Name : String.Join("/", path, file.Name);

            if (!file.IsFolder)
            {
                file.Name = _path;
                files_list.Add(file);
                return;
            }

            var files = Task.Run(() => supabase.Storage.From("Public").List(path: _path)).GetAwaiter().GetResult();

            Debug.WriteLine(_path);

            foreach (Supabase.Storage.FileObject _files in files)
            {
                recursiveSupabaseFileListing(files_list, _files, _path);
            }
        }

        public Task<byte[]> DownloadFile(string fileName, System.ComponentModel.BackgroundWorker backgroundWorker) 
        {
            // TODO: Check if supabase is initilized before proceeding
            var bytes = Task.Run(() => supabase.Storage
            .From("Public")
                        .Download(fileName, (sender, progress) => backgroundWorker.ReportProgress((int)progress, fileName)));
            return bytes;
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

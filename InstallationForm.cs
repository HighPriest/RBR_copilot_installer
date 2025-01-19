using System.Diagnostics;
using System.ComponentModel;

namespace Pacenotes_Installer
{
    internal partial class InstallationForm : Form
    {
        private DownloadManager downloadManager;
        System.ComponentModel.ComponentResourceManager resources = new System.ComponentModel.ComponentResourceManager(typeof(InstallationForm));

        public InstallationForm(DownloadManager _downloadManager)
        {
            InitializeComponent();
            downloadManager = _downloadManager;
            InitializeTabUI(); // Hides tabs from the user
        }

        private void InitializeTabUI()
        {
            // Source: https://www.codeproject.com/Questions/614157/How-to-Hide-TabControl-Headers
            // Set the Appearance to FlatButtons
            tabControl1.Appearance = TabAppearance.FlatButtons;

            // Hide the tab headers
            tabControl1.ItemSize = new Size(0, 1);

            // Adjust the TabControl size
            tabControl1.Dock = DockStyle.Fill;

            // Select Sizing Mode
            tabControl1.SizeMode = TabSizeMode.Fixed;
        }

        private void Form1_Load(object sender, EventArgs e)
        {
            downloadManager.InitializeSupabase();
        }

        private void btn_back_Click(object sender, EventArgs e)
        {
            tabControl1.SelectTab(tabControl1.SelectedIndex - 1);
        }

        private void btn_next_Click(object sender, EventArgs e)
        {
            tabControl1.SelectTab(tabControl1.SelectedIndex + 1);
        }

        #region tab1

        private void tab1buttonNext_Click(object sender, EventArgs e)
        {
            btn_next_Click(sender, e);
            tab2labelStatus.Text = downloadManager.status >= Status.connected ? "Online" : "Offline";
        }

        #endregion
        #region tab2

        private void tab2buttonNext_Click(object sender, EventArgs e)
        {
            if (tab2contentKey.Text != downloadManager.privateKey)
            {
                downloadManager.InitializeSupabase(tab2contentKey.Text);
            }
            btn_next_Click(sender, e);
            if (downloadManager.status < Status.files_listed && downloadManager.status >= Status.connected)
            {
                // if correct code is passed, then the status gets reverted below flies_listed.
                // So, we don't have to worry that the user passes the screen without putting in code, then comes back to put the code in.
                workerListFiles.RunWorkerAsync();
            };
        }

        private void tab2buttonBack_Click(object sender, EventArgs e)
        {
            btn_back_Click(sender, e);
        }

        #endregion
        #region tab3

        private void tab3buttonBrowse_Click(object sender, EventArgs e)
        {
            var btn = sender as Button;
            if (btn == null) { return; }

            FolderBrowserDialog fbd = new FolderBrowserDialog();
            fbd.AutoUpgradeEnabled = true;
            DialogResult result = fbd.ShowDialog();

            if (result == DialogResult.OK && !string.IsNullOrWhiteSpace(fbd.SelectedPath))
            {
                Debug.WriteLine(fbd.SelectedPath); // Just show selected path in debug console
                switch (btn.Tag)
                {
                    case "RSF_DIRECTORY":
                        // Check if correct directory was selected, by querying for games executable
                        if (File.Exists(Path.Combine(fbd.SelectedPath, "RichardBurnsRally_SSE.exe")))
                            tab3dirRBR.Text = fbd.SelectedPath;
                        else
                            System.Windows.Forms.MessageBox.Show(resources.GetString("ErrorRBRDirectory"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                        break;

                    case "CC_DIRECTORY":
                        // Check if correct directory was selected, by querying for CrewChief executable
                        if (File.Exists(Path.Combine(fbd.SelectedPath, "CrewChiefV4.exe")))
                            tab3dirCC.Text = fbd.SelectedPath;
                        else
                            System.Windows.Forms.MessageBox.Show(resources.GetString("ErrorCCDirectory"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                        break;

                    case "AC_DIRECTORY":
                        // Check if correct directory was selected, by querying for games executable
                        if (File.Exists(Path.Combine(fbd.SelectedPath, "AssettoCorsa.exe")))
                            System.Windows.Forms.MessageBox.Show(resources.GetString("ErrorACDirectory"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                        else
                            System.Windows.Forms.MessageBox.Show(resources.GetString("ErrorACNoContent"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                        break;

                    case "DR2_DIRECTORY":
                        // Check if correct directory was selected, by querying for games executable
                        if (File.Exists(Path.Combine(fbd.SelectedPath, "dirtrally2.exe")))
                            System.Windows.Forms.MessageBox.Show(resources.GetString("ErrorDR2Directory"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                        else
                            System.Windows.Forms.MessageBox.Show(resources.GetString("ErrorDR2NoContent"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                        break;
                }
            }
        }

        private void tab3buttonNext_Click(object sender, EventArgs e)
        {
            if (string.IsNullOrEmpty(tab3dirRBR.Text) && string.IsNullOrEmpty(tab3dirAC.Text) && string.IsNullOrEmpty(tab3dirDR2.Text) && string.IsNullOrEmpty(tab3dirCC.Text))
            {
                System.Windows.Forms.MessageBox.Show(text: resources.GetString("ErrorNoDirectory"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                return;
            }
            btn_next_Click(sender, e);
        }

        private void tab3buttonBack_Click(object sender, EventArgs e)
        {
            btn_back_Click(sender, e);
        }

        #endregion
        #region tab4

        private void tab4buttonNext_Click(object sender, EventArgs e)
        {
            // Check if each category has something selected
            foreach (TreeNode node in tab4treeView1.Nodes)
            {
                // If the entire category is checked. Just continue.
                // WARNING: This is error prone! One can select the category & uncheck individual options.
                // TODO: Either make sure the category is unchecked, when no nodes are selected. Or don't do this skip.
                // if (node.Checked) { continue; }

                // If there is 1 or more options in the category, continue the program
                if (node.Nodes.Count > 0)
                {
                    // Check if at least one option is selected in the category
                    if (!tab4AtLeastOneNodeChecked(node))
                    {
                        System.Windows.Forms.MessageBox.Show(text: resources.GetString("ErrorDownloadList"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Exclamation);
                        return;
                    };
                }
            }
            // Wipe Configuration tab & prepare it for new content, based on what is being downloaded
            languageDataSource.Clear(); voiceDataSource.Clear(); styleDataSource.Clear();
            workerDownload.RunWorkerAsync();
            btn_next_Click(sender, e);
        }

        private bool tab4AtLeastOneNodeChecked(TreeNode _node)
        {
            bool status = false;
            if (_node.Nodes.Count > 0)
            {
                foreach (TreeNode node in _node.Nodes)
                {
                    status = tab4AtLeastOneNodeChecked(node);
                    if (status) { break; }
                }
            }
            if (_node.Checked == true)
            {
                return true;
            }

            return status;

        }
        private void tab4treeView1_CheckChildNodes(object sender, TreeViewEventArgs e)
        {
            // Check if the node has children
            if (e.Node.Nodes.Count > 0)
            {
                // Set the checked state of all child nodes to match the parent node
                foreach (TreeNode childNode in e.Node.Nodes)
                {
                    childNode.Checked = e.Node.Checked;
                }
            }
        }

        private void tab4buttonBack_Click(object sender, EventArgs e)
        {
            btn_back_Click(sender, e);
        }

        #endregion
        #region tab5

        private void workerDownload_DoWork(object sender, DoWorkEventArgs e)
        {
            // (Download) Downloads files & does backup of current configuration
            // Traverse all level 0 nodes and try to do download action on them
            foreach (TreeNode treeNode in tab4treeView1.Nodes)
            {
                downloadBasedOnTreeNode((TreeNode)treeNode);
            }

            foreach (string file in Directory.EnumerateFiles(Path.Combine(tab3dirRBR.Text, "backup\\FilipekMod"), "*", SearchOption.AllDirectories))
            {
                string downloadDir = file.Replace(Path.Combine(tab3dirRBR.Text, "backup\\FilipekMod\\"), "");
                string category = downloadDir.Split("\\")[0];
                // Add the file to configuration window
                switch (category.ToLower())
                {
                    case "language":
                        tab6selectLanguage.Invoke(() => languageDataSource.Add(new Pacenotes_Installer.Classes.Language { Name = file.Split("\\").Last().Split(".").First(), Path = downloadDir, FileObject = null }));
                        break;
                    case "sounds":
                        tab6selectLanguage.Invoke(() => voiceDataSource.Add(new Pacenotes_Installer.Classes.Voice { Name = file.Split("\\").Last().Split(".").First(), Path = downloadDir, FileObject = null }));
                        break;
                    case "config":
                        tab6selectStyle.Invoke(() => styleDataSource.Add(new Pacenotes_Installer.Classes.Style { Name = file.Split("\\").Last().Split(".").First(), Path = downloadDir, FileObject = null }));
                        break;
                }
            }
        }

        private void workerDownload_ProgressChanged(object sender, ProgressChangedEventArgs e)
        {
            tab5progressBar1.Value = e.ProgressPercentage;
            tab5label1.Text = e.UserState.ToString();
        }

        private void workerDownload_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            tabControl1.SelectTab(tabControl1.SelectedIndex + 1);
            System.Windows.Forms.MessageBox.Show(text: "Download completed succesfully!", caption: "SUCCESS", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Asterisk);
        }

        #endregion
        #region tab6

        private void tab6buttonNext_Click(object sender, EventArgs e)
        {
            workerInstallation.RunWorkerAsync();
            btn_next_Click(sender, e);
        }

        private void tab6buttonBack_Click(object sender, EventArgs e)
        {
            // Do reset tasks
            tabControl1.SelectTab(0);
        }

        #endregion
        #region tab7

        private void workerInstallation_DoWork(object sender, DoWorkEventArgs e)
        {
            // (Unpack) Unpack all downloaded files into a single directory for bulk installation
            downloadManager.unpackRBRConfiguration(tab3dirRBR.Text, workerInstallation);
            // Directory.Move(Path.Combine(tab3dirRBR.Text, "backup\\FilipekMod"), tab3dirRBR.Text);

            // (Backup) Create a backup of files which we are going to replace
            // Note: This doesn't touch the files which are not going to be replaced
            downloadManager.backupRBRConfiguration(tab3dirRBR.Text, workerInstallation);

            // (Installation) Copy files to working directory, after download & backup
            downloadManager.installRBRConfiguration(tab3dirRBR.Text, workerInstallation);
        }

        private void workerInstallation_ProgressChanged(object sender, ProgressChangedEventArgs e)
        {
            tab7progressBar1.Value = e.ProgressPercentage;
            tab7label1.Text = e.UserState.ToString();
        }

        private void workerInstallation_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            // Do saving tasks
            string config = DownloadManager.CreateRBRConfiguration(tab3dirRBR.Text,
                ((Pacenotes_Installer.Classes.Voice)tab6selectCoPilot.SelectedItem).Path.TrimStart("sounds\\").Split(".")[0], // Or .Remove(0,7)
                ((Pacenotes_Installer.Classes.Style)tab6selectStyle.SelectedItem).Name,
                ((Pacenotes_Installer.Classes.Language)tab6selectLanguage.SelectedItem).Name);
            tab8textConfig.Text = config;

            downloadManager.installRBRPackage(tab3dirRBR.Text, ((Pacenotes_Installer.Classes.Voice)tab6selectCoPilot.SelectedItem).Path, workerInstallation);
            downloadManager.installRBRPackage(tab3dirRBR.Text, ((Pacenotes_Installer.Classes.Style)tab6selectStyle.SelectedItem).Path, workerInstallation);
            downloadManager.installRBRPackage(tab3dirRBR.Text, ((Pacenotes_Installer.Classes.Language)tab6selectLanguage.SelectedItem).Path, workerInstallation);
            tabControl1.SelectTab(tabControl1.SelectedIndex + 1);
            System.Windows.Forms.MessageBox.Show(text: "Installation completed succesfully!", caption: "SUCCESS", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Asterisk);
        }

        #endregion
        #region tab8

        private void tab8buttonNext_Click(object sender, EventArgs e)
        {
            // TODO: Do cleanup of downloaded zip files
            // TODO: Delete both temp & FilipekMod directories from backup
            // When I add checking of LastWrite on zips, stop deleting them, so they can be used as cache
            workerListFiles.Dispose();
            workerDownload.Dispose();
            workerInstallation.Dispose();
            Close();
        }

        private void tab8buttonBack_Click(object sender, EventArgs e)
        {
            // TODO: Maybe do some installation cleanup
            tabControl1.SelectTab(tabControl1.SelectedIndex - 2);
        }

        #endregion

        private void listAllFiles(object sender, System.ComponentModel.DoWorkEventArgs e)
        {
            downloadManager.ListAllFiles();
        }

        private void initializeFileList(object sender, System.ComponentModel.RunWorkerCompletedEventArgs e)
        {
            tab4treeView1.Nodes.Clear();
            TreeNode rootNode = new TreeNode("Root");

            foreach (var node in downloadManager.public_files)
            {
                addNodeToFileList(rootNode, node);
            }
            foreach (TreeNode node in rootNode.Nodes)
            {
                tab4treeView1.Nodes.Add(node);
            }

        }

        private void addNodeToFileList(TreeNode parentNode, Supabase.Storage.FileObject file)
        {
            List<string> segments = new List<string>(file.Name.Split("/"));
            segments[^1] = segments[^1].Replace(".zip", "").Replace(".ini", "");

            foreach (string segment in segments)
            {
                TreeNode childNode = parentNode.Nodes.ContainsKey(segment) ? parentNode.Nodes[segment] : null;

                if (childNode == null)
                {
                    // TreeNodes have 3 data slots: Text, Name, Tag!! Can't populate Name or Tag on creation!
                    // Tag: Supabase.Storage.FileObject
                    // ?? Name = Key. Maybe the extension is unnecessary, but the Key will be needed ?? Name: fileNameWithExtension
                    // Text: fileNameWithout Extension
                    childNode = new TreeNode(segment);
                    childNode.Name = segment;
                    if (segment == segments.Last())
                    {
                        // childNode.Name = pathSegment.Split("/").Last();
                        childNode.Tag = file;
                    }
                    parentNode.Nodes.Add(childNode);
                }
                parentNode.ExpandAll();
                parentNode = childNode;
            }
        }

        private void downloadBasedOnTreeNode(TreeNode treeNode)
        {
            // This is a recursive function.
            // It is initialized for each category tag in the download list
            // Then, it checks whether the node selected is checked & whether it contains information about file to download
            // This means that category tags are skipped & the if check doesn't pass
            if (treeNode.Checked & treeNode.Tag is Supabase.Storage.FileObject)
            {
                Supabase.Storage.FileObject file = (Supabase.Storage.FileObject)treeNode.Tag;
                ListViewItem item = new ListViewItem(treeNode.Text);
                item.Name = treeNode.Name;
                item.Tag = treeNode.Tag;

                // download the file to storage directory
                downloadManager.saveFile(Path.Combine(tab3dirRBR.Text, "backup\\FilipekMod"), (Supabase.Storage.FileObject)treeNode.Tag, workerDownload);

                // Exit the function & don't check for subNodes of node which was a file
                return;
                // This return might not be necessary (I believe).
                // But it assumes that things that are downloadable (files) don't have subNodes, because only directories can have subNodes
            }
            // When the node is not checked & not a category tag
            // Same function is triggered for all subNodes of current node.
            // If the node doesn't have any subNodes, this is just skipped, so edge nodes don't have any issues here
            foreach (TreeNode node in treeNode.Nodes)
            {
                downloadBasedOnTreeNode((TreeNode)node);
            }
        }


        private void textBox1_TextChanged(object sender, EventArgs e)
        {

        }
    }
}
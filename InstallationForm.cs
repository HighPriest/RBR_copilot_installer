using System.Diagnostics;
using System.Xml.Linq;

namespace Pacenotes_Installer
{
    internal partial class InstallationForm : Form
    {
        private DownloadManager downloadManager;
        public InstallationForm(DownloadManager _downloadManager)
        {
            InitializeComponent();
            downloadManager = _downloadManager;
            //InitializeTabUI();
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

        private void tab1buttonNext_Click(object sender, EventArgs e)
        {
            btn_next_Click(sender, e);
            tab2labelStatus.Text = downloadManager.status >= Status.connected ? "Online" : "Offline";
        }

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

        private void tab3buttonBrowse_Click(object sender, EventArgs e)
        {
            var btn = sender as Button;
            if (btn == null) { return; }

            FolderBrowserDialog fbd = new FolderBrowserDialog();
            //fbd.AutoUpgradeEnabled = true;
            DialogResult result = fbd.ShowDialog();

            if (result == DialogResult.OK && !string.IsNullOrWhiteSpace(fbd.SelectedPath))
            {
                if (btn.Tag is "RSF_DIRECTORY")
                {
                    if (File.Exists(Path.Combine(fbd.SelectedPath, "richardburnsrally.exe")))
                        tab3dirRBR.Text = fbd.SelectedPath;
                    else
                        System.Windows.Forms.MessageBox.Show("This is not a correct RSF RBR directory. 'richardburnsrally.exe' is missing");
                }
                if (btn.Tag is "CC_DIRECTORY")
                {
                    Debug.WriteLine(Path.Combine(fbd.SelectedPath, "CrewChiefV4.exe"));
                    if (File.Exists(Path.Combine(fbd.SelectedPath, "CrewChiefV4.exe")))
                        tab3dirCC.Text = fbd.SelectedPath;
                    else
                        System.Windows.Forms.MessageBox.Show("This is not a correct CrewChief directory. 'CrewChiefV4.exe' is missing");
                }
            }
        }

        private void tab3buttonNext_Click(object sender, EventArgs e)
        {
            if (string.IsNullOrEmpty(tab3dirRBR.Text) && string.IsNullOrEmpty(tab3dirCC.Text))
            {
                System.Windows.Forms.MessageBox.Show(text: "No installation directories have been selected!", caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                return;
            }
            btn_next_Click(sender, e);
        }

        private void tab3buttonBack_Click(object sender, EventArgs e)
        {
            btn_back_Click(sender, e);
        }

        private void tab4buttonNext_Click(object sender, EventArgs e)
        {
            workerInstallation.RunWorkerAsync();
            btn_next_Click(sender, e);
        }

        private void tab4buttonBack_Click(object sender, EventArgs e)
        {
            btn_back_Click(sender, e);
        }

        private void tab6buttonNext_Click(object sender, EventArgs e)
        {
            // Do saving tasks
            btn_next_Click(sender, e);
        }

        private void tab6buttonBack_Click(object sender, EventArgs e)
        {
            // Do reset tasks
            tabControl1.SelectedIndex = 0;
        }

        private void tab7buttonNext_Click(object sender, EventArgs e)
        {
            // Do closing tasks
        }

        private void tab7buttonBack_Click(object sender, EventArgs e)
        {
            btn_back_Click(sender, e);
        }

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
                addNodeToFileList(rootNode, node.Name);
            }
            foreach (TreeNode node in rootNode.Nodes)
            {
                tab4treeView1.Nodes.Add(node);
            }
            
        }

        private void addNodeToFileList(TreeNode parentNode, string pathSegment)
        {
            List<string> segments = new List<string>(pathSegment.Split("/"));
            segments[^1] = segments[^1].Replace(".zip", "").Replace(".ini", "");

            foreach (string segment in segments)
            {
                TreeNode childNode = parentNode.Nodes.ContainsKey(segment) ? parentNode.Nodes[segment] : null;

                if (childNode == null)
                {
                    // TreeNodes have 3 data slots: Text, Name, Tag!! Can't populate Name or Tag on creation!
                    childNode = new TreeNode(segment);
                    childNode.Name = segment;
                    if (segment == segments.Last()) { childNode.Tag = pathSegment; }
                    parentNode.Nodes.Add(childNode);
                }

                parentNode = childNode;
            }
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

        private void workerInstallation_DoWork(object sender, System.ComponentModel.DoWorkEventArgs e)
        {
            // Traverse all level 0 nodes and try to do download action on them
            foreach (TreeNode treeNode in tab4treeView1.Nodes)
            {
                downloadBasedOnTreeNode((TreeNode)treeNode);
            }
            
        }

        private void downloadBasedOnTreeNode(TreeNode treeNode)
        {
            if (treeNode.Checked & treeNode.Nodes.Count == 0)
            {
                downloadManager.installFile(Path.Combine("/", tab3dirRBR.Text, "test"), treeNode.Tag.ToString(), workerInstallation);
                return;
            }
            foreach (TreeNode node in treeNode.Nodes)
            {
                downloadBasedOnTreeNode((TreeNode)node);
            }
        }

        private void workerInstallation_ProgressChanged(object sender, System.ComponentModel.ProgressChangedEventArgs e)
        {
            tab5progressBar1.Value = e.ProgressPercentage;
            tab5label1.Text = e.UserState.ToString();
        }

        private void workerInstallation_RunWorkerCompleted(object sender, System.ComponentModel.RunWorkerCompletedEventArgs e)
        {
            tabControl1.SelectTab(tabControl1.SelectedIndex + 1);
            System.Windows.Forms.MessageBox.Show(text: "Installation completed succesfully!", caption: "SUCCESS", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Asterisk);
        }
    }
}

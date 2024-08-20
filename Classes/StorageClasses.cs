using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace Pacenotes_Installer.Classes
{
    using System.Collections.Generic;
    using System.Security.Cryptography.X509Certificates;

    #region RbrConfigurationsStorage
    internal class RbrConfigurationsStorage
    {
        public Configurations? Configurations = new();
        public Sounds? Sounds = new();
    }

    public class Configurations
    {
        public Dictionary<string, List<Supabase.Storage.FileObject>>? Configuration = [];
    }

    public class Sounds
    {
        public Dictionary<string, List<Supabase.Storage.FileObject>>? Sound = [];
    }
    #endregion RbrConfigurationsStorage

    #region CrewChiefConfigurationsStorage
    internal class CrewChiefConfigurationsStorage {
        // TODO: ToImplement
    }


    #endregion CrewChiefConfigurationsStorage

}

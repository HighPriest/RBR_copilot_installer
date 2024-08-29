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
        public Languages? Languages = new();
    }
    public class Configuration
    {
        public string? Id;
    }

    public class Voice
    {
        public string? Id;
    }
    public class Language
    {
        public Dictionary<string, Voice>? Voices = [];
    }

    public class Configurations
    {
        public Dictionary<string, Configuration>? Configuration = [];
    }

    public class Languages
    {
        public Dictionary<string, Language>? Language = [];
    }
    #endregion RbrConfigurationsStorage

    #region CrewChiefConfigurationsStorage
    internal class CrewChiefConfigurationsStorage {
        // TODO: ToImplement
    }


    #endregion CrewChiefConfigurationsStorage

}

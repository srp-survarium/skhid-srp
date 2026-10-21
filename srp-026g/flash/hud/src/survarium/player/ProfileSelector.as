package survarium.player
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.IndexEvent;
   import survarium.Util;
   import survarium.constants.Events;
   import survarium.constants.ProfileState;
   import survarium.data.LocalizationData;
   
   public class ProfileSelector extends MovieClip
   {
      
      public var local:Boolean = true;
      
      public var tabPage:ButtonBar;
      
      public var selectedTabGlow:MovieClip;
      
      protected var _profiles:Array = [];
      
      protected var _selectedProfile:uint = 0;
      
      public function ProfileSelector()
      {
         super();
         this.selectedTabGlow.mouseEnabled = false;
         this.selectedTabGlow.visible = false;
      }
      
      public function getSelectedProfileData() : Object
      {
         return this._profiles[this._selectedProfile];
      }
      
      public function getProfileData(value:uint) : Object
      {
         return this._profiles[value];
      }
      
      public function selectProfile(value:uint) : void
      {
         _selectedProfile = value;
         trace("PS:selectProfile",value,this.tabPage.selectedIndex);
         this.tabPage.removeEventListener(IndexEvent.INDEX_CHANGE,this.onProfileSelected);
         if(this.tabPage.selectedIndex != value)
         {
            this.tabPage.selectedIndex = value;
            this.tabPage.validateNow();
         }
         this.tabPage.addEventListener(IndexEvent.INDEX_CHANGE,this.onProfileSelected);
         this.updateTabGlow();
         dispatchEvent(new Event(Events.PROFILE_SELECTED));
         trace("PS:selectProfile:End");
      }
      
      private function updateTabGlow() : void
      {
         this.selectedTabGlow.visible = true;
         this.selectedTabGlow.x = this.tabPage.x + this.tabPage.selectedIndex * this.tabPage.buttonWidth - 51;
      }
      
      public function setupProfiles(p:Array) : void
      {
         _profiles = p;
         trace("PL::setupProfiles",this._profiles);
         var i:uint = 0;
         var profileName:String;
         while(i < this._profiles.length)
         {
            this._profiles[i].index = i;
            profileName = "«" + this._profiles[i].name + "»";
            if(this._profiles[i].state == ProfileState.PREMIUM_ONLY)
            {
               this._profiles[i].label = Util.styledChar(this._profiles[i].name,"highlight");
               this._profiles[i].levelLabel = "";
               this._profiles[i].tipText = LocalizationData.getName("premium_profile_hint");
            }
            else
            {
               this._profiles[i].levelLabel = this._profiles[i].state == ProfileState.INCOMPLETE ? "" : LocalizationData.getName("clan_players_level_label") + " " + int(this._profiles[i].level).toString();
               this._profiles[i].label = profileName;
               if(this._profiles[i].state == ProfileState.INCOMPLETE && this._profiles[i].hint_text)
               {
                  this._profiles[i].tipText = this._profiles[i].hint_text;
               }
               else
               {
                  this._profiles[i].tipText = this._profiles[i].label + "\n" + Util.styledChar(LocalizationData.getName("player_equipment_level") + " " + int(this._profiles[i].level).toString(),"highlight");
               }
            }
            this._profiles[i].editable = this.local;
            trace("ProfileCreated",this._profiles[i].state);
            i++;
         }
         this.tabPage.removeEventListener(IndexEvent.INDEX_CHANGE,this.onProfileSelected);
         this.tabPage.dataProvider = new DataProvider(this._profiles);
         this.tabPage.validateNow();
         this.tabPage.addEventListener(IndexEvent.INDEX_CHANGE,this.onProfileSelected);
      }
      
      protected function acceptCreateProfie(value:TextField = null) : void
      {
      }
      
      protected function onProfileSelected(e:Event) : void
      {
         trace("PS:onProfileSelected",this.tabPage.selectedIndex);
         if(this.tabPage.selectedIndex != -1)
         {
            _selectedProfile = this.tabPage.selectedIndex;
            if(this.local)
            {
               stackTrace("calledProfileSelected");
               trace("ExternalInterface.call profile_changed");
               ExternalInterface.call("profile_changed",this.tabPage.selectedIndex);
            }
            dispatchEvent(new Event(Events.PROFILE_SELECTED));
         }
         this.updateTabGlow();
      }
   }
}


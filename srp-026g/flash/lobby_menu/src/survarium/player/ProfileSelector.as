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
      
      public function getProfileData(param1:uint) : Object
      {
         return this._profiles[param1];
      }
      
      public function selectProfile(param1:uint) : void
      {
         _selectedProfile = param1;
         trace("PS:selectProfile",param1,this.tabPage.selectedIndex);
         this.tabPage.removeEventListener(IndexEvent.INDEX_CHANGE,this.onProfileSelected);
         if(this.tabPage.selectedIndex != param1)
         {
            this.tabPage.selectedIndex = param1;
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
      
      public function setupProfiles(param1:Array) : void
      {
         _profiles = param1;
         trace("PL::setupProfiles",this._profiles);
         var _loc2_:uint = 0;
         var _loc3_:String;
         while(_loc2_ < this._profiles.length)
         {
            this._profiles[_loc2_].index = _loc2_;
            _loc3_ = "«" + this._profiles[_loc2_].name + "»";
            if(this._profiles[_loc2_].state == 2)
            {
               this._profiles[_loc2_].label = Util.styledChar(this._profiles[_loc2_].name,"highlight");
               this._profiles[_loc2_].levelLabel = "";
               this._profiles[_loc2_].tipText = LocalizationData.getName("premium_profile_hint");
            }
            else
            {
               this._profiles[_loc2_].levelLabel = this._profiles[_loc2_].state == 1 ? "" : LocalizationData.getName("clan_players_level_label") + " " + int(this._profiles[_loc2_].level).toString();
               this._profiles[_loc2_].label = _loc3_;
               if(this._profiles[_loc2_].state == 1 && this._profiles[_loc2_].hint_text)
               {
                  this._profiles[_loc2_].tipText = this._profiles[_loc2_].hint_text;
               }
               else
               {
                  this._profiles[_loc2_].tipText = this._profiles[_loc2_].label + "\n" + Util.styledChar(LocalizationData.getName("player_equipment_level") + " " + int(this._profiles[_loc2_].level).toString(),"highlight");
               }
            }
            this._profiles[_loc2_].editable = this.local;
            trace("ProfileCreated",this._profiles[_loc2_].state);
            _loc2_++;
         }
         this.tabPage.removeEventListener(IndexEvent.INDEX_CHANGE,this.onProfileSelected);
         this.tabPage.dataProvider = new DataProvider(this._profiles);
         this.tabPage.validateNow();
         this.tabPage.addEventListener(IndexEvent.INDEX_CHANGE,this.onProfileSelected);
      }
      
      protected function acceptCreateProfie(param1:TextField = null) : void
      {
      }
      
      protected function onProfileSelected(param1:Event) : void
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


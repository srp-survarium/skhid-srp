package lobby_quest_fla
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   
   public dynamic class questContent_7 extends MovieClip
   {
      
      public var scrollListReputations:SrvButtonBar;
      
      public var scrollList:SrvScrollingListQw;
      
      public var activeTabGlow:MovieClip;
      
      public var selectedTabGlow:SelectedTabGlowP;
      
      public function questContent_7()
      {
         super();
         this.__setProp_scrollList_questContent_();
         this.__setProp_scrollListReputations_questContent_();
      }
      
      internal function __setProp_scrollList_questContent_() : *
      {
         try
         {
            this.scrollList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.scrollList.draggable = false;
         this.scrollList.enabled = true;
         this.scrollList.enableInitCallback = false;
         this.scrollList.focusable = true;
         this.scrollList.itemRendererName = "SrvQuestItemRenderer";
         this.scrollList.itemRendererInstanceName = "";
         this.scrollList.margin = 0;
         this.scrollList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         this.scrollList.rowHeight = 213;
         this.scrollList.scrollBar = "SrvScrollBarSkinned";
         this.scrollList.scrollYPadding = 0;
         this.scrollList.visible = true;
         this.scrollList.wrapping = "normal";
         try
         {
            this.scrollList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_scrollListReputations_questContent_() : *
      {
         try
         {
            this.scrollListReputations["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.scrollListReputations.autoSize = "none";
         this.scrollListReputations.buttonWidth = 141;
         this.scrollListReputations.direction = "horizontal";
         this.scrollListReputations.enabled = true;
         this.scrollListReputations.enableInitCallback = false;
         this.scrollListReputations.focusable = true;
         this.scrollListReputations.itemRendererName = "SrvQuestFactionTab";
         this.scrollListReputations.spacing = 0;
         this.scrollListReputations.visible = true;
         try
         {
            this.scrollListReputations["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}


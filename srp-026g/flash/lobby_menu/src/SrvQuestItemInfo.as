package
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
   import survarium.controls.QuestItemRenderer;
   
   public dynamic class SrvQuestItemInfo extends QuestItemRenderer
   {
      
      public function SrvQuestItemInfo()
      {
         super();
         addFrameScript(0,this.frame1);
         this.__setProp_rewardBar_SrvQuestItemInfo_back_1_0();
         this.__setProp_taskList_SrvQuestItemInfo_back_1_0();
      }
      
      internal function __setProp_rewardBar_SrvQuestItemInfo_back_1_0() : *
      {
         try
         {
            rewardBar["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         rewardBar.autoSize = "none";
         rewardBar.buttonWidth = 0;
         rewardBar.direction = "horizontal";
         rewardBar.enabled = true;
         rewardBar.enableInitCallback = false;
         rewardBar.focusable = true;
         rewardBar.itemRendererName = "SrvRewardButtonShared";
         rewardBar.spacing = 0;
         rewardBar.visible = true;
         try
         {
            rewardBar["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_taskList_SrvQuestItemInfo_back_1_0() : *
      {
         try
         {
            taskList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         taskList.autoSize = "none";
         taskList.buttonWidth = 0;
         taskList.direction = "vertical";
         taskList.enabled = true;
         taskList.enableInitCallback = false;
         taskList.focusable = true;
         taskList.itemRendererName = "SrvTaskRendererShort";
         taskList.spacing = 0;
         taskList.visible = true;
         try
         {
            taskList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function frame1() : *
      {
      }
   }
}


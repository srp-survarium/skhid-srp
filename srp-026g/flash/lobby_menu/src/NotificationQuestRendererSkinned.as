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
   import survarium.buttons.ButtonNotificationQuestLog;
   
   public dynamic class NotificationQuestRendererSkinned extends ButtonNotificationQuestLog
   {
      
      public function NotificationQuestRendererSkinned()
      {
         super();
         addFrameScript(9,this.frame10,19,this.frame20,29,this.frame30,39,this.frame40);
         this.__setProp_rewardBar_NotificationQuestRenderer_content_0();
      }
      
      internal function __setProp_rewardBar_NotificationQuestRenderer_content_0() : *
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
      
      internal function frame10() : *
      {
         stop();
      }
      
      internal function frame20() : *
      {
         stop();
      }
      
      internal function frame30() : *
      {
         stop();
      }
      
      internal function frame40() : *
      {
         stop();
      }
   }
}


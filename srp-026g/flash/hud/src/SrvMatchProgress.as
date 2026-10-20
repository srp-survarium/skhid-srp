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
   
   public dynamic class SrvMatchProgress extends MovieClip
   {
      
      public var team2LastPlayers:SrvSectionStatusIndicator;
      
      public var team1Progress:TextField;
      
      public var team1LastPlayers:SrvSectionStatusIndicator;
      
      public var art_teams:MovieClip;
      
      public var match_time_2_tdm:DefaultLabel3;
      
      public var team2Progress:TextField;
      
      public var match_time_2:DefaultLabel2;
      
      public var team_2_score:DefaultStatusIndicatorRed;
      
      public var team_1_score:DefaultStatusIndicator;
      
      public var team_2_progress:DefaultLabel4;
      
      public var respawn_time:DefaultLabel33;
      
      public var capture_progress_bar:SrvCaptureBar;
      
      public var capture_progress_bar_team_2:DefaultStatusIndicator;
      
      public var team_1_progress:DefaultLabel34;
      
      public var capture_progress_bar_team_1:DefaultStatusIndicator;
      
      public var team_1:DefaultLabel12;
      
      public var team_2:DefaultLabel33;
      
      public var __setPropDict:Dictionary = new Dictionary(true);
      
      public var __lastFrameProp:int = -1;
      
      public function SrvMatchProgress()
      {
         super();
         addFrameScript(0,this.frame1,1,this.frame2,2,this.frame3,4,this.frame5);
         addEventListener(Event.FRAME_CONSTRUCTED,this.__setProp_handler,false,0,true);
      }
      
      internal function __setProp_capture_progress_bar_team_1_progress_indicator_sprite_movies_0() : *
      {
         if(this.__setPropDict[this.capture_progress_bar_team_1] == undefined || int(this.__setPropDict[this.capture_progress_bar_team_1]) != 1)
         {
            this.__setPropDict[this.capture_progress_bar_team_1] = 1;
            try
            {
               this.capture_progress_bar_team_1["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.capture_progress_bar_team_1.enabled = true;
            this.capture_progress_bar_team_1.enableInitCallback = false;
            this.capture_progress_bar_team_1.maximum = 100;
            this.capture_progress_bar_team_1.minimum = 0;
            this.capture_progress_bar_team_1.value = 0;
            this.capture_progress_bar_team_1.visible = true;
            try
            {
               this.capture_progress_bar_team_1["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_capture_progress_bar_team_2_progress_indicator_sprite_movies_0() : *
      {
         if(this.__setPropDict[this.capture_progress_bar_team_2] == undefined || int(this.__setPropDict[this.capture_progress_bar_team_2]) != 1)
         {
            this.__setPropDict[this.capture_progress_bar_team_2] = 1;
            try
            {
               this.capture_progress_bar_team_2["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.capture_progress_bar_team_2.enabled = true;
            this.capture_progress_bar_team_2.enableInitCallback = false;
            this.capture_progress_bar_team_2.maximum = 100;
            this.capture_progress_bar_team_2.minimum = 0;
            this.capture_progress_bar_team_2.value = 0;
            this.capture_progress_bar_team_2.visible = true;
            try
            {
               this.capture_progress_bar_team_2["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_respawn_time_progress_indicator_sprite_movies_2(curFrame:int) : *
      {
         if(this.respawn_time != null && curFrame >= 3 && curFrame <= 5 && (this.__setPropDict[this.respawn_time] == undefined || !(int(this.__setPropDict[this.respawn_time]) >= 3 && int(this.__setPropDict[this.respawn_time]) <= 5)))
         {
            this.__setPropDict[this.respawn_time] = curFrame;
            try
            {
               this.respawn_time["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.respawn_time.autoSize = "center";
            this.respawn_time.enabled = true;
            this.respawn_time.enableInitCallback = false;
            this.respawn_time.text = "";
            this.respawn_time.visible = true;
            try
            {
               this.respawn_time["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_team_1_progress_indicator_sprite_movies_2(curFrame:int) : *
      {
         if(this.team_1 != null && curFrame >= 3 && curFrame <= 4 && (this.__setPropDict[this.team_1] == undefined || !(int(this.__setPropDict[this.team_1]) >= 3 && int(this.__setPropDict[this.team_1]) <= 4)))
         {
            this.__setPropDict[this.team_1] = curFrame;
            try
            {
               this.team_1["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.team_1.autoSize = "none";
            this.team_1.enabled = true;
            this.team_1.enableInitCallback = false;
            this.team_1.text = "";
            this.team_1.visible = false;
            try
            {
               this.team_1["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_team_2_progress_indicator_sprite_movies_2(curFrame:int) : *
      {
         if(this.team_2 != null && curFrame >= 3 && curFrame <= 4 && (this.__setPropDict[this.team_2] == undefined || !(int(this.__setPropDict[this.team_2]) >= 3 && int(this.__setPropDict[this.team_2]) <= 4)))
         {
            this.__setPropDict[this.team_2] = curFrame;
            try
            {
               this.team_2["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.team_2.autoSize = "none";
            this.team_2.enabled = true;
            this.team_2.enableInitCallback = false;
            this.team_2.text = "";
            this.team_2.visible = false;
            try
            {
               this.team_2["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_team_2_score_progress_indicator_sprite_movies_4() : *
      {
         if(this.__setPropDict[this.team_2_score] == undefined || int(this.__setPropDict[this.team_2_score]) != 5)
         {
            this.__setPropDict[this.team_2_score] = 5;
            try
            {
               this.team_2_score["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.team_2_score.enabled = true;
            this.team_2_score.enableInitCallback = false;
            this.team_2_score.maximum = 15;
            this.team_2_score.minimum = 0;
            this.team_2_score.value = 0;
            this.team_2_score.visible = true;
            try
            {
               this.team_2_score["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_team1LastPlayers_progress_indicator_sprite_movies_4() : *
      {
         if(this.__setPropDict[this.team1LastPlayers] == undefined || int(this.__setPropDict[this.team1LastPlayers]) != 5)
         {
            this.__setPropDict[this.team1LastPlayers] = 5;
            try
            {
               this.team1LastPlayers["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.team1LastPlayers.enabled = true;
            this.team1LastPlayers.enableInitCallback = false;
            this.team1LastPlayers.maximum = 16;
            this.team1LastPlayers.minimum = 0;
            this.team1LastPlayers.value = 0;
            this.team1LastPlayers.visible = true;
            try
            {
               this.team1LastPlayers["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_team2LastPlayers_progress_indicator_sprite_movies_4() : *
      {
         if(this.__setPropDict[this.team2LastPlayers] == undefined || int(this.__setPropDict[this.team2LastPlayers]) != 5)
         {
            this.__setPropDict[this.team2LastPlayers] = 5;
            try
            {
               this.team2LastPlayers["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.team2LastPlayers.enabled = true;
            this.team2LastPlayers.enableInitCallback = false;
            this.team2LastPlayers.maximum = 16;
            this.team2LastPlayers.minimum = 0;
            this.team2LastPlayers.value = 0;
            this.team2LastPlayers.visible = false;
            try
            {
               this.team2LastPlayers["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_team_1_score_progress_indicator_sprite_movies_4() : *
      {
         if(this.__setPropDict[this.team_1_score] == undefined || int(this.__setPropDict[this.team_1_score]) != 5)
         {
            this.__setPropDict[this.team_1_score] = 5;
            try
            {
               this.team_1_score["componentInspectorSetting"] = true;
            }
            catch(e:Error)
            {
            }
            this.team_1_score.enabled = true;
            this.team_1_score.enableInitCallback = false;
            this.team_1_score.maximum = 15;
            this.team_1_score.minimum = 0;
            this.team_1_score.value = 0;
            this.team_1_score.visible = false;
            try
            {
               this.team_1_score["componentInspectorSetting"] = false;
            }
            catch(e:Error)
            {
            }
         }
      }
      
      internal function __setProp_handler(e:Object) : *
      {
         var curFrame:int = currentFrame;
         if(this.__lastFrameProp == curFrame)
         {
            return;
         }
         __lastFrameProp = curFrame;
         this.__setProp_respawn_time_progress_indicator_sprite_movies_2(curFrame);
         this.__setProp_team_1_progress_indicator_sprite_movies_2(curFrame);
         this.__setProp_team_2_progress_indicator_sprite_movies_2(curFrame);
      }
      
      internal function frame1() : *
      {
         this.__setProp_capture_progress_bar_team_2_progress_indicator_sprite_movies_0();
         this.__setProp_capture_progress_bar_team_1_progress_indicator_sprite_movies_0();
         stop();
      }
      
      internal function frame2() : *
      {
      }
      
      internal function frame3() : *
      {
         this.team_1.visible = false;
         this.team_2.visible = false;
      }
      
      internal function frame5() : *
      {
         this.__setProp_team_1_score_progress_indicator_sprite_movies_4();
         this.__setProp_team2LastPlayers_progress_indicator_sprite_movies_4();
         this.__setProp_team1LastPlayers_progress_indicator_sprite_movies_4();
         this.__setProp_team_2_score_progress_indicator_sprite_movies_4();
         this.team1LastPlayers.registerRenderer(WhiteDead);
         this.team2LastPlayers.registerRenderer(RedDead);
         this.team_1_score.visible = false;
         this.team_2_score.visible = false;
         this.team1LastPlayers.visible = false;
         this.team2LastPlayers.visible = false;
         this.team1Progress.visible = false;
         this.team2Progress.visible = false;
      }
   }
}

